//! Depthpad desktop app: the engine behind a Tauri window, tray icon and profile store.

use std::path::PathBuf;

use depthpad_core::engine::{Action, ActionFn, Engine, Snapshot};
use depthpad_core::device::{self, Follow, Model, Status};
use depthpad_core::platform::drivers::{detect, Found};
use depthpad_core::platform::overlay;
use depthpad_core::platform::pad::PadKind;
use depthpad_core::platform::sound::{self, Cue};
use depthpad_core::profile::{builtin, Profile, ProfileStore};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_opener::OpenerExt;

mod foreground;

// ------------------------------------------------------------------------------------------ paths

fn config_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("Depthpad")
}
fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())).join("Depthpad")
}
fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

// ------------------------------------------------------------------------------------------ prefs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    profile: String,
    close_to_tray: bool,
    start_minimized: bool,
    check_updates: bool,
    tray_hint_shown: bool,
    /// Model id of the keyboard to always use; None = pick automatically, ask when several.
    default_keyboard: Option<String>,
    /// Hotkeys as key-name combos ("ControlRight+KeyP"); "" = none. App-wide, not per profile.
    /// `pause_hotkey` starts as None and is taken from the active profile once (older versions
    /// kept it there).
    pause_hotkey: Option<String>,
    next_profile_hotkey: String,
    /// Profile name -> hotkey that switches straight to it.
    profile_hotkeys: std::collections::BTreeMap<String, String>,
    /// How hotkey switches are shown outside the window: "both", "badge", "sound" or "off".
    indicator: String,
    /// The virtual controller: Xbox 360 or DualShock 4.
    controller: PadKind,
    /// Unplug the virtual controller while paused.
    unplug_when_paused: bool,
    /// App linking: program file name ("gta5.exe") -> profile that switches on while it's in
    /// front.
    app_links: std::collections::BTreeMap<String, String>,
    /// Appearance: "dark", "midnight", "oled" or "light", and the accent colour.
    theme: String,
    accent: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { profile: "Default".into(), close_to_tray: true, start_minimized: false, check_updates: true,
               tray_hint_shown: false, default_keyboard: None, pause_hotkey: None,
               next_profile_hotkey: "ControlRight+KeyO".into(), profile_hotkeys: Default::default(),
               indicator: "both".into(), controller: PadKind::Xbox, unplug_when_paused: false,
               app_links: Default::default(), theme: "dark".into(), accent: "#4c8dff".into() }
    }
}

impl Prefs {
    fn path() -> PathBuf {
        config_dir().join("settings.json")
    }
    fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        let tmp = Self::path().with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, Self::path());
        }
    }
}

// ------------------------------------------------------------------------------------------ state

struct App {
    /// The keyboard model the engine runs.
    model: &'static Model,
    engine: Option<Engine>,
    store: ProfileStore,
    active: Profile,
    prefs: Prefs,
    recording: Option<PathBuf>,
    /// Several supported keyboards are connected and none is the default: the window asks.
    needs_choice: bool,
    /// What the engine calls when a profile hotkey fires (set once the app is running).
    on_action: Option<ActionFn>,
    on_pause: Option<std::sync::Arc<dyn Fn(bool) + Send + Sync>>,
    on_identified: Option<std::sync::Arc<dyn Fn(&'static Model) + Send + Sync>>,
}

type AppState = Mutex<App>;

impl App {
    fn hotkeys(&self) -> Vec<(String, Action)> {
        let mut list = vec![
            (self.prefs.pause_hotkey.clone().unwrap_or_default(), Action::Pause),
            (self.prefs.next_profile_hotkey.clone(), Action::NextProfile),
        ];
        for (name, combo) in &self.prefs.profile_hotkeys {
            list.push((combo.clone(), Action::Profile(name.clone())));
        }
        list
    }

    /// Give the engine the current hotkeys and the handlers for them.
    fn wire_engine(&self) {
        if let Some(e) = &self.engine {
            e.set_hotkeys(self.hotkeys());
            if let Some(f) = &self.on_action {
                e.on_action(f.clone());
            }
            if let Some(f) = &self.on_pause {
                e.on_pause(f.clone());
            }
            if let Some(f) = &self.on_identified {
                e.on_identified(f.clone());
            }
            e.set_pad(self.prefs.controller, self.prefs.unplug_when_paused);
        }
    }

    /// Make a profile the active one.
    fn activate(&mut self, name: &str) -> Result<Profile, String> {
        let p = self.store.load(name).map_err(err)?;
        self.active = p.clone();
        self.prefs.profile = name.to_string();
        self.prefs.save();
        self.apply();
        Ok(p)
    }

    fn apply(&mut self) {
        if let Some(e) = &self.engine {
            e.set_profile(self.active.clone());
        }
    }
    /// Switch the engine to another keyboard model (ends any recording: it was for the old one).
    fn use_model(&mut self, model: &'static Model) {
        if model.id != self.model.id {
            self.model = model;
            self.recording = None;
            self.restart_engine(None);
        }
    }

    fn keyboards(&self, found: &[Found]) -> Keyboards {
        let mut list: Vec<KeyboardEntry> = Vec::new();
        for m in connected_models(found).into_iter().chain(std::iter::once(self.model)) {
            // a family whose exact board is known shows as that board
            let m = if m.id == self.model.family_id() { self.model } else { m };
            if !list.iter().any(|k| k.id == m.id) {
                list.push(KeyboardEntry {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    vendor: m.vendor.clone(),
                    status: m.status,
                    connected: found.iter().any(|f| f.model.id == m.family_id()),
                    note: m.note.clone(),
                });
            }
        }
        Keyboards { list, active: self.model.id.clone(), default: self.prefs.default_keyboard.clone(), needs_choice: self.needs_choice }
    }

    fn restart_engine(&mut self, log: Option<PathBuf>) {
        let paused = self.engine.as_ref().is_some_and(|e| e.paused());
        if let Some(mut e) = self.engine.take() {
            e.stop();
        }
        let e = Engine::start(self.model, self.active.clone(), log, true);
        e.set_paused(paused);
        self.engine = Some(e);
        self.wire_engine();
    }
}

// ------------------------------------------------------------------------------------------ keyboards

#[derive(Debug, Clone, PartialEq, Serialize)]
struct KeyboardEntry {
    id: String,
    name: String,
    vendor: String,
    status: Status,
    connected: bool,
    note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Keyboards {
    /// Connected supported keyboards, plus the one in use even when it is unplugged.
    list: Vec<KeyboardEntry>,
    active: String,
    default: Option<String>,
    needs_choice: bool,
}

/// One entry per connected model, in detection order.
fn connected_models(found: &[Found]) -> Vec<&'static Model> {
    device::unique(found.iter().map(|f| f.model))
}

/// Every 2 s: notice keyboards being plugged in or out, tell the window, and (with no default
/// chosen) move to a supported keyboard when the current one is gone.
fn spawn_keyboard_watch(app: AppHandle) {
    std::thread::Builder::new()
        .name("depthpad-keyboards".into())
        .spawn(move || {
            let mut last: Option<Keyboards> = None;
            loop {
                let found = detect();
                let state = {
                    let st = app.state::<AppState>();
                    let mut a = st.lock();
                    let has_default = a.prefs.default_keyboard.is_some();
                    match device::follow(a.model, &connected_models(&found), has_default) {
                        Follow::Stay => {}
                        Follow::Switch(m) => a.use_model(m),
                        Follow::Ask => a.needs_choice = true,
                    }
                    a.keyboards(&found)
                };
                if last.as_ref() != Some(&state) {
                    let _ = app.emit("keyboards", &state);
                    last = Some(state);
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        })
        .expect("spawn keyboard watch");
}

#[tauri::command]
fn keyboards(state: State<AppState>) -> Keyboards {
    let found = detect();
    state.lock().keyboards(&found)
}

/// Use a keyboard now; with `remember`, also make it the default.
#[tauri::command]
fn use_keyboard(app: AppHandle, state: State<AppState>, id: String, remember: bool) -> Result<Keyboards, String> {
    let model = device::model(&id).ok_or_else(|| format!("unknown keyboard {id:?}"))?;
    let found = detect();
    let mut a = state.lock();
    if remember {
        a.prefs.default_keyboard = Some(id);
        a.prefs.save();
    }
    a.needs_choice = false;
    a.use_model(model);
    let k = a.keyboards(&found);
    let _ = app.emit("keyboards", &k);
    Ok(k)
}

/// Make a keyboard the default, or (None) go back to picking automatically.
#[tauri::command]
fn set_default_keyboard(app: AppHandle, state: State<AppState>, id: Option<String>) -> Keyboards {
    let found = detect();
    let mut a = state.lock();
    a.prefs.default_keyboard = id.filter(|i| device::model(i).is_some());
    a.prefs.save();
    if let Some(m) = a.prefs.default_keyboard.as_deref().and_then(device::model) {
        a.needs_choice = false;
        a.use_model(m);
    }
    let k = a.keyboards(&found);
    let _ = app.emit("keyboards", &k);
    k
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

// ------------------------------------------------------------------------------------------ commands

#[derive(Serialize)]
struct KeyEntry {
    pos: u8,
    key: String,
    hid: u16,
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    profile_dir: String,
    log_dir: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        profile_dir: config_dir().join("profiles").display().to_string(),
        log_dir: log_dir().display().to_string(),
    }
}

#[tauri::command]
fn keys(state: State<AppState>) -> Vec<KeyEntry> {
    let model = state.lock().model;
    model.keys.iter().map(|(p, k)| KeyEntry { pos: *p, key: k.key.clone(), hid: k.hid }).collect()
}

/// The keyboard model in use: name, layout and keys.
#[tauri::command]
fn device(state: State<AppState>) -> &'static Model {
    state.lock().model
}

#[tauri::command]
fn snapshot(state: State<AppState>) -> Option<Snapshot> {
    state.lock().engine.as_ref().map(|e| e.snapshot())
}

#[tauri::command]
fn set_paused(state: State<AppState>, paused: bool) {
    if let Some(e) = &state.lock().engine {
        e.set_paused(paused);
    }
}

#[tauri::command]
fn profiles(state: State<AppState>) -> Vec<String> {
    state.lock().store.names()
}

#[tauri::command]
fn active_profile(state: State<AppState>) -> Profile {
    state.lock().active.clone()
}

/// Save edits to the active profile and apply them live.
#[tauri::command]
fn save_profile(state: State<AppState>, profile: Profile) -> Result<(), String> {
    let problems = profile.validate();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let mut app = state.lock();
    if profile.name != app.active.name {
        return Err("only the active profile can be edited".into());
    }
    app.store.save(&profile).map_err(err)?;
    app.active = profile;
    app.apply();
    Ok(())
}

#[tauri::command]
fn activate_profile(state: State<AppState>, name: String) -> Result<Profile, String> {
    state.lock().activate(&name)
}

#[tauri::command]
fn new_profile(state: State<AppState>, from_active: bool) -> Result<Profile, String> {
    let app = state.lock();
    let mut p = if from_active { app.active.clone() } else { builtin("Default") };
    let base = if from_active { format!("{} copy", app.active.name) } else { "New profile".to_string() };
    p.name = app.store.unique_name(&base);
    app.store.save(&p).map_err(err)?;
    drop(app);
    activate_profile(state, p.name.clone())
}

#[tauri::command]
fn rename_profile(state: State<AppState>, name: String) -> Result<Profile, String> {
    let name = name.trim().to_string();
    let app = state.lock();
    if name.is_empty() || name == app.active.name {
        return Ok(app.active.clone());
    }
    if app.store.names().contains(&name) {
        return Err(format!("A profile named “{name}” already exists."));
    }
    let old = app.active.name.clone();
    let mut p = app.active.clone();
    p.name = name.clone();
    app.store.save(&p).map_err(err)?;
    app.store.delete(&old).map_err(err)?;
    let mut app = app;
    if let Some(h) = app.prefs.profile_hotkeys.remove(&old) {
        app.prefs.profile_hotkeys.insert(name.clone(), h);
    }
    for target in app.prefs.app_links.values_mut() {
        if *target == old {
            *target = name.clone();
        }
    }
    let p = app.activate(&name)?;
    app.wire_engine();
    Ok(p)
}

#[tauri::command]
fn delete_profile(state: State<AppState>) -> Result<Profile, String> {
    let app = state.lock();
    let names = app.store.names();
    if names.len() <= 1 {
        return Err("This is the only profile, so it can't be deleted.".into());
    }
    let gone = app.active.name.clone();
    app.store.delete(&gone).map_err(err)?;
    let next = names.into_iter().find(|n| *n != gone).unwrap();
    let mut app = app;
    app.prefs.profile_hotkeys.remove(&gone);
    app.prefs.app_links.retain(|_, t| *t != gone);
    let p = app.activate(&next)?;
    app.wire_engine();
    Ok(p)
}

#[tauri::command]
fn import_profile(state: State<AppState>, path: String) -> Result<Profile, String> {
    let mut p = ProfileStore::load_file(std::path::Path::new(&path)).map_err(|e| format!("Couldn't read that file: {e}"))?;
    let problems = p.validate();
    if !problems.is_empty() {
        return Err(format!("That profile has problems: {}", problems.join("; ")));
    }
    let app = state.lock();
    p.name = app.store.unique_name(&p.name);
    app.store.save(&p).map_err(err)?;
    drop(app);
    activate_profile(state, p.name)
}

#[tauri::command]
fn export_profile(state: State<AppState>, path: String) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&state.lock().active).map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

#[tauri::command]
fn builtin_layout(name: String) -> Profile {
    builtin(&name)
}

#[tauri::command]
fn prefs(state: State<AppState>) -> serde_json::Value {
    serde_json::to_value(&state.lock().prefs).unwrap_or_default()
}

#[tauri::command]
fn set_pref(state: State<AppState>, key: String, value: bool) {
    let mut app = state.lock();
    match key.as_str() {
        "close_to_tray" => app.prefs.close_to_tray = value,
        "start_minimized" => app.prefs.start_minimized = value,
        "check_updates" => app.prefs.check_updates = value,
        _ => return,
    }
    app.prefs.save();
}

/// How switches are shown outside the window.
#[tauri::command]
fn set_indicator(state: State<AppState>, mode: String) {
    if ["both", "badge", "sound", "off"].contains(&mode.as_str()) {
        let mut app = state.lock();
        app.prefs.indicator = mode;
        app.prefs.save();
    }
}

/// Set the pause or next-profile hotkey ("" = none).
#[tauri::command]
fn set_hotkey(state: State<AppState>, which: String, combo: String) {
    let mut app = state.lock();
    match which.as_str() {
        "pause" => app.prefs.pause_hotkey = Some(combo),
        "next" => app.prefs.next_profile_hotkey = combo,
        _ => return,
    }
    app.prefs.save();
    app.wire_engine();
}

/// Set the hotkey that switches to a profile ("" = none).
#[tauri::command]
fn set_profile_hotkey(state: State<AppState>, name: String, combo: String) {
    let mut app = state.lock();
    if combo.is_empty() {
        app.prefs.profile_hotkeys.remove(&name);
    } else {
        app.prefs.profile_hotkeys.insert(name, combo);
    }
    app.prefs.save();
    app.wire_engine();
}

/// Hotkeys off while the window records a new one (so the combo reaches the window).
#[tauri::command]
fn suspend_hotkeys(state: State<AppState>, suspended: bool) {
    if let Some(e) = &state.lock().engine {
        e.suspend_hotkeys(suspended);
    }
}

/// Show a switch outside the window (badge and/or sound), unless the window is in front.
fn indicate(app: &AppHandle, cue: Cue, title: &str, detail: &str) {
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false) && w.is_visible().unwrap_or(false));
    if in_front {
        return;
    }
    let mode = app.state::<AppState>().lock().prefs.indicator.clone();
    if mode == "both" || mode == "badge" {
        overlay::show(title, detail);
    }
    if mode == "both" || mode == "sound" {
        sound::play(cue);
    }
}

/// Switch to a profile from outside the window (hotkey, app link), and tell the window, the
/// tray and (if a game is in front) the player.
fn switch_to(app: &AppHandle, target: &str) {
    let st = app.state::<AppState>();
    let mut a = st.lock();
    let names = a.store.names();
    if target == a.active.name || !names.iter().any(|n| n == target) {
        return;
    }
    if let Ok(p) = a.activate(target) {
        drop(a);
        let index = names.iter().position(|n| *n == p.name).unwrap_or(0) + 1;
        indicate(app, Cue::Profile(index), &p.name, &format!("Profile {index} of {}", names.len()));
        if let Some(tray) = app.tray_by_id("tray") {
            let _ = tray.set_tooltip(Some(format!("Depthpad · {}", p.name)));
        }
        let _ = app.emit("profile-switched", &p);
    }
}

/// A profile hotkey fired.
fn on_hotkey(app: &AppHandle, action: Action) {
    let target = {
        let st = app.state::<AppState>();
        let a = st.lock();
        let names = a.store.names();
        match action {
            Action::NextProfile => {
                let i = names.iter().position(|n| *n == a.active.name).map_or(0, |i| (i + 1) % names.len().max(1));
                names.get(i).cloned()
            }
            Action::Profile(name) => Some(name),
            Action::Pause => None,
        }
    };
    if let Some(t) = target {
        switch_to(app, &t);
    }
}

/// App linking: once a second, see which program is in front; switch to its linked profile, and
/// back to the previous profile when it isn't in front any more. Depthpad's own window doesn't
/// count (opening it to look must not switch anything).
fn spawn_app_links(app: AppHandle) {
    std::thread::Builder::new()
        .name("depthpad-app-links".into())
        .spawn(move || {
            // the linked program in front, and the profile to go back to
            let mut linked: Option<(String, String)> = None;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let Some(exe) = foreground::foreground_exe() else { continue };
                if exe == "depthpad.exe" {
                    continue;
                }
                let (target, current) = {
                    let st = app.state::<AppState>();
                    let a = st.lock();
                    (a.prefs.app_links.get(&exe).cloned(), a.active.name.clone())
                };
                match (target, &linked) {
                    (Some(profile), Some((was, _))) if *was == exe => {
                        let _ = profile; // still in front: leave any manual switch alone
                    }
                    (Some(profile), _) => {
                        let back = linked.take().map_or(current, |(_, back)| back);
                        switch_to(&app, &profile);
                        linked = Some((exe, back));
                    }
                    (None, Some(_)) => {
                        let (_, back) = linked.take().unwrap();
                        switch_to(&app, &back);
                    }
                    (None, None) => {}
                }
            }
        })
        .expect("spawn app links");
}

/// Programs with a window, for choosing one to link.
#[tauri::command]
fn running_apps() -> Vec<foreground::RunningApp> {
    foreground::running_apps()
}

/// Link a program to a profile ("" removes the link). `exe` may be a path or a file name.
#[tauri::command]
fn set_app_link(state: State<AppState>, exe: String, profile: String) {
    let exe = foreground::exe_name(&exe);
    if exe.is_empty() {
        return;
    }
    let mut a = state.lock();
    if profile.is_empty() {
        a.prefs.app_links.remove(&exe);
    } else {
        a.prefs.app_links.insert(exe, profile);
    }
    a.prefs.save();
}

/// Theme and accent colour.
#[tauri::command]
fn set_appearance(state: State<AppState>, theme: String, accent: String) {
    let ok_theme = ["dark", "midnight", "oled", "light"].contains(&theme.as_str());
    let ok_accent = accent.len() == 7 && accent.starts_with('#') && accent[1..].chars().all(|c| c.is_ascii_hexdigit());
    if ok_theme && ok_accent {
        let mut a = state.lock();
        a.prefs.theme = theme;
        a.prefs.accent = accent;
        a.prefs.save();
    }
}

/// The virtual controller kind, and whether to unplug it while paused.
#[tauri::command]
fn set_controller(state: State<AppState>, kind: PadKind, unplug_when_paused: bool) {
    let mut a = state.lock();
    a.prefs.controller = kind;
    a.prefs.unplug_when_paused = unplug_when_paused;
    a.prefs.save();
    a.wire_engine();
}

/// Start or stop a session recording; returns the file while recording, or the saved file.
#[tauri::command]
fn is_recording(state: State<AppState>) -> bool {
    state.lock().recording.is_some()
}

#[tauri::command]
fn toggle_recording(state: State<AppState>) -> (bool, Option<String>) {
    let mut app = state.lock();
    if let Some(path) = app.recording.take() {
        app.restart_engine(None);
        (false, Some(path.display().to_string()))
    } else {
        let _ = std::fs::create_dir_all(log_dir());
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = log_dir().join(format!("session-{stamp}.log"));
        app.restart_engine(Some(path.clone()));
        app.recording = Some(path.clone());
        (true, Some(path.display().to_string()))
    }
}

#[tauri::command]
fn open_folder(app: AppHandle, which: String) {
    let dir = if which == "logs" { log_dir() } else { config_dir().join("profiles") };
    let _ = std::fs::create_dir_all(&dir);
    let _ = app.opener().open_path(dir.display().to_string(), None::<&str>);
}

#[tauri::command]
fn vigembus_installed() -> bool {
    std::path::Path::new(r"C:\Windows\System32\drivers\ViGEmBus.sys").exists()
}

/// Run the bundled ViGEmBus installer (it asks Windows for permission itself).
#[tauri::command]
fn install_vigembus(app: AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .resolve("redist/ViGEmBus_1.22.0_x64_x86_arm64.exe", tauri::path::BaseDirectory::Resource)
        .map_err(err)?;
    if !path.exists() {
        return Err("The ViGEmBus installer isn't included in this build.".into());
    }
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("Start-Process -FilePath '{}' -ArgumentList '/passive','/norestart' -Verb RunAs", path.display())])
        .spawn()
        .map(|_| ())
        .map_err(err)
}

/// Open the keyboard report tool in its own console window.
#[tauri::command]
fn create_keyboard_report(app: AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    let bundled = app.path().resolve("depthpad-probe.exe", tauri::path::BaseDirectory::Resource).ok().filter(|p| p.exists());
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("depthpad-probe.exe"))).filter(|p| p.exists());
    let exe = bundled.or(beside).ok_or("The keyboard report tool isn't included in this build.")?;
    std::process::Command::new(exe).creation_flags(CREATE_NEW_CONSOLE).spawn().map(|_| ()).map_err(err)
}

// ------------------------------------------------------------------------------------------ app

/// Make the window behave like an app, not a web page: no browser context menu, no browser
/// shortcuts (Ctrl+P print, Ctrl+R reload, Ctrl+F find, F5, zoom...). Dev builds keep them for
/// debugging.
#[cfg(windows)]
fn app_like_webview(w: &tauri::WebviewWindow) {
    if cfg!(debug_assertions) {
        return;
    }
    let _ = w.with_webview(|wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;
        let Ok(core) = wv.controller().CoreWebView2() else { return };
        let Ok(settings) = core.Settings() else { return };
        let _ = settings.SetAreDefaultContextMenusEnabled(false);
        let _ = settings.SetIsZoomControlEnabled(false);
        let _ = settings.SetIsStatusBarEnabled(false);
        if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
            let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
        }
    });
}

/// Push engine snapshots to the window instead of having it poll: up to 60 a second, but only when
/// something visible changed (the update rate itself refreshes once a second), and none while the
/// window is hidden or minimised. An idle window then costs next to nothing.
fn spawn_ui_feed(app: AppHandle) {
    std::thread::Builder::new()
        .name("depthpad-ui-feed".into())
        .spawn(move || {
            let mut last_sig = String::new();
            let mut last_rate = (std::time::Instant::now(), 0.0);
            loop {
                let shown = app.get_webview_window("main").is_some_and(|w| {
                    w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false)
                });
                if !shown {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    continue;
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
                let snap = app.state::<AppState>().lock().engine.as_ref().map(|e| e.snapshot());
                let Some(mut snap) = snap else { continue };
                let rate_due = last_rate.0.elapsed().as_millis() >= 1000;
                if rate_due {
                    last_rate = (std::time::Instant::now(), snap.rate);
                } else {
                    snap.rate = last_rate.1;
                }
                let sig = serde_json::to_string(&snap).unwrap_or_default();
                if sig != last_sig {
                    let _ = app.emit("snapshot", &snap);
                    last_sig = sig;
                }
            }
        })
        .expect("spawn ui feed");
}

/// Show the window, creating it if needed. The window only exists while it is open: closing it to
/// the tray destroys it, which frees the WebView (well over 100 MB) and leaves just the engine
/// running while you play.
fn show_main(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let w = match handle.get_webview_window("main") {
            Some(w) => w,
            None => {
                let Some(cfg) = handle.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
                let Ok(w) = tauri::WebviewWindowBuilder::from_config(&handle, &cfg).and_then(|b| b.build()) else { return };
                #[cfg(windows)]
                app_like_webview(&w);
                w
            }
        };
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    });
}

pub fn run() {
    let saved = Prefs::load();
    let store = ProfileStore::open(config_dir().join("profiles")).expect("profile folder");
    let active = store.load(&saved.profile).or_else(|_| store.load(&store.names()[0])).expect("a profile");
    let minimized = std::env::args().any(|a| a == "--minimized") || saved.start_minimized;
    let mut saved = saved;
    if saved.pause_hotkey.is_none() {
        saved.pause_hotkey = Some(active.settings.pause_hotkey.clone());
        saved.save();
    }
    let default = saved.default_keyboard.as_deref().and_then(device::model);
    let (model, needs_choice) = device::pick(&connected_models(&detect()), default, device::fallback());
    let engine = Engine::start(model, active.clone(), None, true);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                                             Some(vec!["--minimized"])))
        .manage(Mutex::new(App { model, needs_choice, on_action: None, on_pause: None, on_identified: None, engine: Some(engine), store, active, prefs: saved, recording: None }))
        .invoke_handler(tauri::generate_handler![
            app_info, keys, device, keyboards, use_keyboard, set_default_keyboard, snapshot, set_paused, profiles, active_profile, save_profile, activate_profile,
            new_profile, rename_profile, delete_profile, import_profile, export_profile, builtin_layout, prefs,
            set_pref, set_hotkey, set_profile_hotkey, suspend_hotkeys, set_indicator, running_apps, set_app_link, set_controller, set_appearance, is_recording, toggle_recording, open_folder, vigembus_installed, install_vigembus, create_keyboard_report
        ])
        .setup(move |app| {
            {
                let handle = app.handle().clone();
                let st = app.state::<AppState>();
                let mut a = st.lock();
                a.on_action = Some(std::sync::Arc::new(move |action| on_hotkey(&handle, action)));
                let handle = app.handle().clone();
                a.on_pause = Some(std::sync::Arc::new(move |paused| {
                    if paused {
                        indicate(&handle, Cue::Paused, "Paused", "Controller neutral, keys type normally");
                    } else {
                        let name = handle.state::<AppState>().lock().active.name.clone();
                        indicate(&handle, Cue::Resumed, "Active", &name);
                    }
                }));
                let handle = app.handle().clone();
                a.on_identified = Some(std::sync::Arc::new(move |variant| {
                    // switching restarts the engine, which waits for the driver thread we're on
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        let found = detect();
                        let st = handle.state::<AppState>();
                        let mut a = st.lock();
                        if a.model.family_id() == variant.family_id() {
                            a.use_model(variant);
                            let k = a.keyboards(&found);
                            drop(a);
                            let _ = handle.emit("keyboards", &k);
                        }
                    });
                }));
                a.wire_engine();
            }
            spawn_ui_feed(app.handle().clone());
            spawn_keyboard_watch(app.handle().clone());
            spawn_app_links(app.handle().clone());
            let open = MenuItem::with_id(app, "open", "Open Depthpad", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause / Resume", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &pause, &PredefinedMenuItem::separator(app)?, &quit])?;
            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("Depthpad")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "pause" => {
                        let st = app.state::<AppState>();
                        let app_ = st.lock();
                        if let Some(e) = &app_.engine {
                            e.set_paused(!e.paused());
                        }
                    }
                    "quit" => {
                        if let Some(mut e) = app.state::<AppState>().lock().engine.take() {
                            e.stop();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;
            if !minimized {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let st = window.state::<AppState>();
                let mut app = st.lock();
                if app.prefs.close_to_tray {
                    // let the window close (and free its WebView); ExitRequested below keeps the
                    // app and engine running in the tray
                    let _ = api;
                    app.prefs.tray_hint_shown = true;
                    app.prefs.save();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Depthpad")
        .run(|app, event| {
            // the last window closing asks to exit (code None); with close-to-tray, stay running
            if let RunEvent::ExitRequested { code: None, api, .. } = &event {
                if app.state::<AppState>().lock().prefs.close_to_tray {
                    api.prevent_exit();
                    return;
                }
            }
            if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
                if let Some(mut e) = app.state::<AppState>().lock().engine.take() {
                    e.stop(); // analog stream off, controller unplugged
                }
            }
        });
}

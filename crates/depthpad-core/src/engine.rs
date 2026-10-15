//! The running gamepad: keyboard stream + key events -> fusion -> mapper -> virtual pad.
//!
//! Threads: stream (keyboard session), rawinput (this keyboard's key events), hook (optional
//! blocking and the pause hotkey), output (~1 kHz report to the virtual pad), logwriter
//! (optional session recording, off the realtime path, in the format tools/check_log.py reads).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;

use crate::device::Model;
use crate::fusion::{Fusion, Source};
use crate::mapper::{Mapper, Report};
use crate::platform::drivers::{self, Connected, Session, StreamSink, StreamState};
use crate::platform::hook::KeyBlocker;
use crate::platform::now;
use crate::platform::pad::{Pad, PadKind};
use crate::platform::rawinput::DeviceKeys;
use crate::profile::Profile;
use crate::protocols::Sample;

/// What a hotkey does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Action {
    /// Pause or resume the controller (and blocking).
    Pause,
    /// Switch to the next profile.
    NextProfile,
    /// Switch to the named profile.
    Profile(String),
}

/// Called (on its own thread) when a hotkey other than pause fires.
pub type ActionFn = Arc<dyn Fn(Action) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct KeyView {
    pub pos: u8,
    pub depth: f64,
    pub source: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub state: StreamState,
    pub error: Option<String>,
    pub firmware: Option<String>,
    pub rate: f64,
    pub keys: Vec<KeyView>,
    pub output: Report,
    pub paused: bool,
    pub blocking: bool,
    pub pad: bool,
    pub pad_error: Option<String>,
    /// Which controller is (or would be) plugged in.
    pub pad_kind: PadKind,
    /// Unplugged on purpose while paused.
    pub pad_parked: bool,
    pub contention: bool,
    pub last_event: Option<String>,
}

struct Inner {
    model: &'static Model,
    fusion: Mutex<Fusion>,
    mapper: Mutex<Mapper>,
    profile: Mutex<Profile>,
    usage_to_pos: Mutex<HashMap<u16, u8>>,
    pos_to_usage: Mutex<BTreeMap<u8, u16>>,
    paused: AtomicBool,
    /// Usages held according to this keyboard's key events, for the backup hotkey check.
    held: Mutex<HashSet<u16>>,
    /// Hotkeys as key-name combos ("ControlRight+KeyP"); index 0 is always pause.
    hotkeys: Mutex<Vec<(String, Action)>>,
    /// Hotkeys off, e.g. while the window records a new one.
    hotkeys_suspended: AtomicBool,
    on_action: Mutex<Option<ActionFn>>,
    /// Told (on its own thread) whenever pausing changes, from any source.
    on_pause: Mutex<Option<Arc<dyn Fn(bool) + Send + Sync>>>,
    events: Mutex<VecDeque<String>>,
    output: Mutex<Report>,
    firmware: Mutex<Option<String>>,
    pad_ok: AtomicBool,
    pad_error: Mutex<Option<String>>,
    pad_kind: Mutex<PadKind>,
    /// Unplug the virtual controller while paused (for games that ignore the mouse while a
    /// controller is connected).
    unplug_paused: AtomicBool,
    pad_parked: AtomicBool,
    blocker: Mutex<Option<KeyBlocker>>,
    log: Mutex<Option<Sender<String>>>,
    t0: f64,
}

impl Inner {
    fn log(&self, t: f64, line: impl FnOnce() -> String) {
        if let Some(tx) = self.log.lock().as_ref() {
            let _ = tx.send(format!("{:9.1} {}", (t - self.t0) * 1000.0, line()));
        }
    }

    fn event(&self, text: String) {
        self.log(now(), || format!("E {text}"));
        let mut ev = self.events.lock();
        ev.push_back(text);
        while ev.len() > 100 {
            ev.pop_front();
        }
    }

    fn on_key(&self, pos: u8, down: bool, t: f64) {
        self.log(t, || format!("K {pos} {}", down as u8));
        self.fusion.lock().on_digital(pos, down, t);
    }

    fn on_usage(&self, usage: u16, down: bool, t: f64) {
        self.backup_hotkey(usage, down);
        let pos = self.usage_to_pos.lock().get(&usage).copied();
        if let Some(pos) = pos {
            self.on_key(pos, down, t);
        }
    }

    /// Pause or resume from any source, keeping the hook's view in step.
    fn set_paused(&self, paused: bool) {
        if self.paused.swap(paused, Ordering::SeqCst) != paused {
            if let Some(b) = self.blocker.lock().as_ref() {
                b.set_paused(paused);
            }
            self.event(if paused { "paused".into() } else { "resumed".into() });
            self.paused_changed(paused);
        }
    }

    fn paused_changed(&self, paused: bool) {
        if let Some(f) = self.on_pause.lock().clone() {
            std::thread::spawn(move || f(paused));
        }
    }

    /// The hotkeys as usages for the hook, in the same order (unusable combos -> empty).
    fn hotkey_usages(&self) -> Vec<Vec<u16>> {
        if self.hotkeys_suspended.load(Ordering::SeqCst) {
            return Vec::new();
        }
        self.hotkeys
            .lock()
            .iter()
            .map(|(combo, _)| {
                let usages: Option<Vec<u16>> = combo
                    .split('+')
                    .filter(|k| !k.is_empty())
                    .map(|k| self.model.position_of(k).map(|p| self.usage_of(p)))
                    .collect();
                usages.unwrap_or_default()
            })
            .collect()
    }

    /// Hotkey `i` fired: pause toggles here; anything else goes to the app, off this thread
    /// (the hook's thread must answer Windows quickly, and switching profiles reconfigures it).
    fn dispatch(&self, i: usize, from_hook: bool) {
        let Some(action) = self.hotkeys.lock().get(i).map(|h| h.1.clone()) else { return };
        if action == Action::Pause {
            if !from_hook {
                self.set_paused(!self.paused.load(Ordering::SeqCst));
            }
            return;
        }
        self.event(format!("hotkey: {}", match &action {
            Action::NextProfile => "next profile".to_string(),
            Action::Profile(name) => format!("profile {name}"),
            Action::Pause => unreachable!(),
        }));
        if let Some(f) = self.on_action.lock().clone() {
            std::thread::spawn(move || f(action));
        }
    }

    /// Hotkeys seen through this keyboard's own key events. The hook normally handles a hotkey
    /// and swallows its last key, so that key never reaches here; it only does when Windows has
    /// removed the hook (it silently drops hooks that answer too slowly). So this fires exactly
    /// when the hook couldn't, and never twice for one press.
    fn backup_hotkey(&self, usage: u16, down: bool) {
        let mut held = self.held.lock();
        if !down {
            held.remove(&usage);
            return;
        }
        if !held.insert(usage) {
            return; // auto-repeat
        }
        let hit = self
            .hotkey_usages()
            .iter()
            .enumerate()
            .filter(|(_, h)| h.split_last().is_some_and(|(last, mods)| *last == usage && mods.iter().all(|m| held.contains(m))))
            .max_by_key(|(_, h)| h.len())
            .map(|(i, _)| i);
        drop(held);
        if let Some(i) = hit {
            self.dispatch(i, false);
        }
    }

    fn usage_of(&self, pos: u8) -> u16 {
        self.pos_to_usage.lock().get(&pos).copied().unwrap_or(self.model.keys[&pos].hid)
    }

    /// (Re)configure blocking and the hotkeys; start the hook if needed.
    fn update_blocker(self: &Arc<Self>) {
        let (bound, block, block_all, alt_tab) = {
            let p = self.profile.lock();
            let m = self.mapper.lock();
            let bound: HashSet<u16> = m.positions.iter().map(|&p| self.usage_of(p)).collect();
            let s = &p.settings;
            (bound, s.block_keys || s.block_all_keys, s.block_all_keys, s.block_allow_alt_tab)
        };
        let hotkeys = self.hotkey_usages();
        let mut slot = self.blocker.lock();
        if let Some(b) = slot.as_ref() {
            b.configure(bound, block, block_all, alt_tab, hotkeys);
            return;
        }
        if !block && hotkeys.iter().all(Vec::is_empty) {
            return;
        }
        let (a, b, c) = (self.clone(), self.clone(), self.clone());
        match KeyBlocker::start(
            bound,
            block,
            block_all,
            alt_tab,
            hotkeys,
            Box::new(move |u, d, t| a.on_usage(u, d, t)),
            Box::new(move |p| {
                b.paused.store(p, Ordering::SeqCst);
                b.event(if p { "paused".into() } else { "resumed".into() });
                b.paused_changed(p);
            }),
            Box::new(move |i| c.dispatch(i, true)),
        ) {
            Ok(k) => *slot = Some(k),
            Err(e) => {
                drop(slot);
                self.event(format!("key blocking unavailable: {e}"));
            }
        }
    }
}

struct Sink(Arc<Inner>);

impl StreamSink for Sink {
    fn sample(&self, s: Sample, t: f64) {
        self.0.log(t, || format!("S {} {}", s.position, s.travel));
        self.0.fusion.lock().on_stream(s.position, s.travel, s.stroke, t);
    }
    fn lost(&self) {
        self.0.fusion.lock().clear_stream();
    }
    fn connected(&self, c: Connected) {
        *self.0.firmware.lock() = c.firmware;
        if !c.actuation.is_empty() {
            let mut mm: Vec<f64> = c.actuation.values().map(|t| t / 100.0).collect();
            mm.sort_by(f64::total_cmp);
            let (lo, hi) = (mm[0], mm[mm.len() - 1]);
            self.0.event(if (hi - lo).abs() < 0.005 {
                format!("actuation point {lo:.2} mm on every key")
            } else {
                format!("actuation points {lo:.2}-{hi:.2} mm")
            });
        }
        self.0.fusion.lock().set_actuation(c.actuation);
        *self.0.usage_to_pos.lock() = reverse(&c.to_usage);
        *self.0.pos_to_usage.lock() = c.to_usage;
        if let Some(note) = c.note {
            self.0.event(note);
        }
        self.0.update_blocker();
    }
    fn event(&self, text: String) {
        self.0.event(text);
    }
    fn key(&self, pos: u8, down: bool, t: f64) {
        self.0.on_key(pos, down, t);
    }
}

/// Usage -> position; when two keys send one usage, the first owns it.
fn reverse(to_usage: &BTreeMap<u8, u16>) -> HashMap<u16, u8> {
    let mut to_pos = HashMap::new();
    for (&pos, &u) in to_usage {
        to_pos.entry(u).or_insert(pos);
    }
    to_pos
}

/// The factory layout: each key's usage as the model file gives it (0 = no usage, e.g. Fn).
fn factory(model: &Model) -> BTreeMap<u8, u16> {
    model.keys.iter().filter(|(_, k)| k.hid != 0).map(|(&p, k)| (p, k.hid)).collect()
}

pub struct Engine {
    inner: Arc<Inner>,
    stream: Option<Box<dyn Session>>,
    keys: Option<DeviceKeys>,
    output: Option<JoinHandle<()>>,
    log_thread: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    rate_window: Mutex<VecDeque<(f64, u64)>>,
    rate: Mutex<f64>,
    rearm_times: Mutex<(VecDeque<f64>, u64)>,
    reclaims_seen: Mutex<u64>,
    hook_reinstalls_seen: Mutex<u64>,
}

impl Engine {
    /// Run the gamepad for a keyboard model. The driver keeps looking for the keyboard until it
    /// is connected.
    pub fn start(model: &'static Model, profile: Profile, log_path: Option<PathBuf>, virtual_pad: bool) -> Self {
        let mapper = Mapper::new(&profile, model);
        let pause_hotkey = profile.settings.pause_hotkey.clone();
        let to_usage = factory(model);
        let to_pos = reverse(&to_usage);
        let inner = Arc::new(Inner {
            model,
            fusion: Mutex::new({
                let mut f = Fusion::default();
                f.unread_full = profile.settings.unread_depth == "full";
                if let Some(ms) = model.sonix.fresh_ms {
                    f.fresh = ms as f64 / 1000.0;
                }
                f
            }),
            mapper: Mutex::new(mapper),
            profile: Mutex::new(profile),
            usage_to_pos: Mutex::new(to_pos),
            pos_to_usage: Mutex::new(to_usage),
            paused: AtomicBool::new(false),
            held: Mutex::new(HashSet::new()),
            hotkeys: Mutex::new(vec![(pause_hotkey, Action::Pause)]),
            hotkeys_suspended: AtomicBool::new(false),
            on_action: Mutex::new(None),
            on_pause: Mutex::new(None),
            events: Mutex::new(VecDeque::new()),
            output: Mutex::new(Report::default()),
            firmware: Mutex::new(None),
            pad_ok: AtomicBool::new(false),
            pad_error: Mutex::new(None),
            pad_kind: Mutex::new(PadKind::Xbox),
            unplug_paused: AtomicBool::new(false),
            pad_parked: AtomicBool::new(false),
            blocker: Mutex::new(None),
            log: Mutex::new(None),
            t0: now(),
        });
        let log_thread = log_path.and_then(|path| {
            let file = File::create(&path).ok()?;
            let (tx, rx) = channel::<String>();
            *inner.log.lock() = Some(tx);
            Some(std::thread::spawn(move || {
                let mut w = BufWriter::new(file);
                for line in rx {
                    let _ = writeln!(w, "{line}");
                }
                let _ = w.flush();
            }))
        });
        inner.update_blocker();
        let (a, b) = (inner.clone(), inner.clone());
        let keys = DeviceKeys::start(
            model,
            Box::new(move |u, d, t| a.on_usage(u, d, t)),
            Box::new(move || {
                b.fusion.lock().release_all();
                b.event("keyboard removed; keys released".into());
            }),
        );
        let stream = drivers::start(model, Arc::new(Sink(inner.clone())));
        let stop = Arc::new(AtomicBool::new(false));
        let (oi, os) = (inner.clone(), stop.clone());
        let output = std::thread::Builder::new()
            .name("depthpad-output".into())
            .spawn(move || output_loop(oi, os, virtual_pad))
            .expect("spawn output thread");
        Self {
            inner,
            stream: Some(stream),
            keys: Some(keys),
            output: Some(output),
            log_thread,
            stop,
            rate_window: Mutex::new(VecDeque::new()),
            rate: Mutex::new(0.0),
            rearm_times: Mutex::new((VecDeque::new(), 0)),
            reclaims_seen: Mutex::new(0),
            hook_reinstalls_seen: Mutex::new(0),
        }
    }

    pub fn model(&self) -> &'static Model {
        self.inner.model
    }

    /// Set the hotkeys. Pause comes first whatever the order given (an empty combo disables one).
    pub fn set_hotkeys(&self, mut list: Vec<(String, Action)>) {
        let pause = list.iter().position(|h| h.1 == Action::Pause).map(|i| list.remove(i));
        list.insert(0, pause.unwrap_or((String::new(), Action::Pause)));
        *self.inner.hotkeys.lock() = list;
        self.inner.update_blocker();
    }

    /// Turn hotkeys off and on (while the window records a new one).
    pub fn suspend_hotkeys(&self, suspended: bool) {
        self.inner.hotkeys_suspended.store(suspended, Ordering::SeqCst);
        self.inner.update_blocker();
    }

    /// Which virtual controller to use, and whether to unplug it while paused. Takes effect
    /// within a millisecond (the controller is re-plugged as the new kind).
    pub fn set_pad(&self, kind: PadKind, unplug_paused: bool) {
        *self.inner.pad_kind.lock() = kind;
        self.inner.unplug_paused.store(unplug_paused, Ordering::SeqCst);
    }

    /// Be told whenever the controller is paused or resumed.
    pub fn on_pause(&self, f: Arc<dyn Fn(bool) + Send + Sync>) {
        *self.inner.on_pause.lock() = Some(f);
    }

    /// What to do when a hotkey other than pause fires.
    pub fn on_action(&self, f: ActionFn) {
        *self.inner.on_action.lock() = Some(f);
    }

    pub fn set_profile(&self, profile: Profile) {
        *self.inner.mapper.lock() = Mapper::new(&profile, self.inner.model);
        self.inner.fusion.lock().unread_full = profile.settings.unread_depth == "full";
        *self.inner.profile.lock() = profile;
        self.inner.update_blocker();
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.set_paused(paused);
    }

    pub fn paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    pub fn events(&self) -> Vec<String> {
        self.inner.events.lock().iter().cloned().collect()
    }

    pub fn snapshot(&self) -> Snapshot {
        let t = now();
        let (status, packets) = match &self.stream {
            Some(s) => (Some(s.status().clone()), s.status().packets.load(Ordering::Relaxed)),
            None => (None, 0),
        };
        {
            let mut w = self.rate_window.lock();
            w.push_back((t, packets));
            while w.len() > 2 && t - w[1].0 >= 1.0 {
                w.pop_front();
            }
            let (t0, p0) = w[0];
            if t - t0 >= 0.25 {
                *self.rate.lock() = (packets - p0) as f64 / (t - t0);
            }
        }
        let contention = status.as_ref().is_some_and(|s| {
            let rearms = s.rearms.load(Ordering::Relaxed);
            let mut r = self.rearm_times.lock();
            if rearms != r.1 {
                r.0.push_back(t);
                r.1 = rearms;
            }
            while r.0.front().is_some_and(|&x| t - x > 15.0) {
                r.0.pop_front();
            }
            r.0.len() >= 2
        });
        if let Some(k) = &self.keys {
            let n = k.reclaims.load(Ordering::Relaxed);
            let mut seen = self.reclaims_seen.lock();
            if n != *seen {
                *seen = n;
                // routine (the window grabs it whenever it is created), so log only
                self.inner.log(t, || "E keyboard raw input reclaimed from another window".to_string());
            }
        }
        if let Some(n) = self.inner.blocker.lock().as_ref().map(|b| b.reinstalls.load(Ordering::Relaxed)) {
            let mut seen = self.hook_reinstalls_seen.lock();
            if n > *seen {
                *seen = n;
                drop(seen);
                self.inner.event("Windows dropped the key hook (pause hotkey, blocking); reinstalled it".into());
            }
        }
        let keys = {
            let f = self.inner.fusion.lock();
            self.inner
                .model
                .keys
                .keys()
                .filter_map(|&pos| {
                    let depth = f.depth(pos, t);
                    let source = f.source(pos, t).map(|s| match s {
                        Source::Stream => "stream",
                        Source::Held => "held",
                        Source::Release => "release",
                    });
                    (depth > 0.0 || source.is_some()).then_some(KeyView { pos, depth, source })
                })
                .collect()
        };
        let paused = self.paused();
        Snapshot {
            state: status.as_ref().map_or(StreamState::Searching, |s| *s.state.lock()),
            error: status.as_ref().and_then(|s| s.error.lock().clone()),
            firmware: self.inner.firmware.lock().clone(),
            rate: *self.rate.lock(),
            keys,
            output: *self.inner.output.lock(),
            paused,
            blocking: { let s = &self.inner.profile.lock().settings; s.block_keys || s.block_all_keys } && !paused && self.inner.blocker.lock().is_some(),
            pad: self.inner.pad_ok.load(Ordering::SeqCst),
            pad_error: self.inner.pad_error.lock().clone(),
            pad_kind: *self.inner.pad_kind.lock(),
            pad_parked: self.inner.pad_parked.load(Ordering::SeqCst),
            contention,
            last_event: self.inner.events.lock().back().cloned(),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.output.take() {
            let _ = t.join();
        }
        if let Some(mut s) = self.stream.take() {
            s.stop(); // turns the analog stream off
        }
        if let Some(mut k) = self.keys.take() {
            k.stop();
        }
        if let Some(mut b) = self.inner.blocker.lock().take() {
            b.stop();
        }
        *self.inner.log.lock() = None;
        if let Some(t) = self.log_thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn output_loop(inner: Arc<Inner>, stop: Arc<AtomicBool>, virtual_pad: bool) {
    use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
    unsafe {
        timeBeginPeriod(1);
    }
    // the controller is (re)plugged here whenever its kind or the pause rules call for it
    let mut pad: Option<(Pad, PadKind)> = None;
    let mut retry_at = 0.0;
    let mut last = None;
    while !stop.load(Ordering::SeqCst) {
        let t = now();
        let report = if inner.paused.load(Ordering::SeqCst) {
            Report::default()
        } else {
            let f = inner.fusion.lock().clone();
            inner.mapper.lock().compute(&f, t)
        };
        *inner.output.lock() = report;
        let kind = *inner.pad_kind.lock();
        let parked = inner.paused.load(Ordering::SeqCst) && inner.unplug_paused.load(Ordering::SeqCst);
        let want = virtual_pad && !parked;
        if pad.as_ref().is_some_and(|(_, k)| !want || *k != kind) {
            pad = None; // neutral report + unplug
            inner.pad_ok.store(false, Ordering::SeqCst);
        }
        inner.pad_parked.store(virtual_pad && parked, Ordering::SeqCst);
        if want && pad.is_none() && t >= retry_at {
            match Pad::connect(kind) {
                Ok(p) => {
                    pad = Some((p, kind));
                    inner.pad_ok.store(true, Ordering::SeqCst);
                    if inner.pad_error.lock().take().is_some() {
                        inner.event("virtual controller connected".into());
                    }
                }
                Err(e) => {
                    retry_at = t + 3.0;
                    let mut err = inner.pad_error.lock();
                    if err.as_deref() != Some(e.as_str()) {
                        *err = Some(e.clone());
                        drop(err);
                        inner.event(format!("virtual controller unavailable: {e}"));
                    }
                }
            }
        }
        if let Some((p, _)) = pad.as_mut() {
            p.update(&report);
        }
        let rounded = [report.buttons as f64, report.lt, report.rt, report.lx, report.ly, report.rx, report.ry]
            .map(|v| (v * 100.0).round() / 100.0);
        if last != Some(rounded) {
            last = Some(rounded);
            inner.log(t, || {
                format!("O {} {} {} {} {} {} {}", report.buttons, rounded[1], rounded[2], rounded[3], rounded[4],
                        rounded[5], rounded[6])
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(pad); // neutral report + unplug
    unsafe {
        timeEndPeriod(1);
    }
}

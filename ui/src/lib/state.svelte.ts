import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
// App-wide reactive state and actions. Components read `app.*` and call the actions.
import { api, applyAppearance, type AppInfo, type KeyEntry, type Prefs, type Profile, type Snapshot, type Device, type Keyboards } from "./api";

export interface Toast { id: number; message: string; action?: { label: string; run: () => void }; error?: boolean }

class AppState {
  snapshot = $state<Snapshot | null>(null);
  profile = $state<Profile | null>(null);
  profiles = $state<string[]>([]);
  prefs = $state<Prefs | null>(null);
  info = $state<AppInfo | null>(null);
  keys = $state<KeyEntry[]>([]);
  /** The keyboard model in use. */
  device = $state<Device | null>(null);
  /** Connected supported keyboards and the default. */
  keyboards = $state<Keyboards | null>(null);
  page = $state<"live" | "mapping" | "settings" | "about">("live");
  /** Key selected on the Mapping page (kept across tab switches). */
  mapKey = $state<number | null>(null);
  toasts = $state<Toast[]>([]);
  vigembus = $state(true);
  recording = $state(false);
  /** A newer release, once found. */
  update = $state<Update | null>(null);
  updating = $state(false);
  /** Output history for the analysis charts: [t, lx, ly, rx, ry, lt, rt] */
  history: number[][] = [];
  historyTick = $state(0);

  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  private toastId = 0;

  /** Full key travel of the keyboard, mm. */
  travel = $derived(this.device?.travel_mm ?? 3.4);
  private legends = $derived(new Map((this.device?.layout ?? []).map((k) => [k.pos, k.legend || "Space"])));
  /** The printed legend of a key position. */
  legend = (pos: number) => this.legends.get(pos) ?? "?";
  keyByPos = $derived(new Map(this.keys.map((k) => [k.pos, k])));
  posByName = $derived(new Map(this.keys.map((k) => [k.key, k.pos])));

  /** Key position -> controller input, for the active profile. */
  bindings = $derived.by(() => {
    const m = new Map<number, string>();
    for (const b of this.profile?.bindings ?? []) {
      const pos = this.posByName.get(b.key);
      if (pos !== undefined && !m.has(pos)) m.set(pos, b.target);
    }
    return m;
  });

  /** Key position -> live depth and its source. */
  depths = $derived(new Map((this.snapshot?.keys ?? []).map((k) => [k.pos, { depth: k.depth, source: k.source }])));

  mappable = (pos: number) => (this.keyByPos.get(pos)?.hid ?? 0) > 0;

  /** Start or stop recording a session log (the engine restarts either way). */
  async toggleRecording() {
    const [on, path] = await api.toggleRecording();
    this.recording = on;
    if (!on && path) this.toast(`Recording saved to ${path}`, { action: { label: "Open folder", run: () => api.openFolder("logs") } });
  }

  async init() {
    [this.info, this.keys, this.device, this.profile, this.profiles, this.prefs, this.vigembus] = await Promise.all([
      api.appInfo(), api.keys(), api.device(), api.activeProfile(), api.profiles(), api.prefs(), api.vigembusInstalled(),
    ]);
    if (this.prefs?.theme) applyAppearance(this.prefs.theme, this.prefs.accent);
    this.recording = await api.isRecording().catch(() => false);
    this.keyboards = await api.keyboards().catch(() => null);
    if (this.prefs?.check_updates) this.checkForUpdate(false);
    listen<Keyboards>("keyboards", (e) => this.onKeyboards(e.payload));
    // a hotkey switched the profile
    listen<Profile>("profile-switched", (e) => {
      this.profile = e.payload;
      if (this.prefs) this.prefs.profile = e.payload.name;
      this.toast(`Switched to ${e.payload.name}.`);
    });
    this.poll();
  }

  /** The keyboard list changed; when the keyboard in use changed too, reload its layout. */
  async onKeyboards(k: Keyboards) {
    this.keyboards = k;
    if (this.device && k.active !== this.device.id) {
      [this.device, this.keys] = await Promise.all([api.device(), api.keys()]);
      this.mapKey = null;
      this.recording = await api.isRecording().catch(() => false);
      this.toast(`Now using ${this.device.name}.`);
    }
  }

  /** Use a keyboard (and optionally make it the default). */
  async useKeyboard(id: string, remember: boolean) {
    try {
      await this.onKeyboards(await api.useKeyboard(id, remember));
    } catch (e) {
      this.toast(String(e), { error: true });
    }
  }

  /** Look for a newer release on GitHub. `manual`: say so when there is none (or it failed). */
  async checkForUpdate(manual: boolean) {
    try {
      this.update = await check();
      if (manual && !this.update) this.toast("Depthpad is up to date.");
    } catch (e) {
      if (manual) this.toast(`Couldn't check for updates: ${e}`, { error: true });
    }
  }

  /** Download and run the signed installer, then restart into the new version. */
  async installUpdate() {
    if (!this.update || this.updating) return;
    this.updating = true;
    try {
      await this.update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      this.updating = false;
      this.toast(`The update failed: ${e}`, { error: true });
    }
  }

  async setHotkey(which: "pause" | "next", combo: string) {
    if (!this.prefs) return;
    if (which === "pause") this.prefs.pause_hotkey = combo;
    else this.prefs.next_profile_hotkey = combo;
    await api.setHotkey(which, combo);
  }

  async setProfileHotkey(name: string, combo: string) {
    if (!this.prefs) return;
    if (combo) this.prefs.profile_hotkeys[name] = combo;
    else delete this.prefs.profile_hotkeys[name];
    await api.setProfileHotkey(name, combo);
  }

  async setDefaultKeyboard(id: string | null) {
    await this.onKeyboards(await api.setDefaultKeyboard(id));
  }

  // The backend pushes a snapshot whenever something visible changes (up to 60 a second, none
  // while idle or hidden). The analysis charts still need time to move while output is steady,
  // so their history is sampled on its own 10 Hz timer, only while the Live page is open.
  private onSnapshot = (s: Snapshot) => {
    this.snapshot = s;
    this.record();
  };
  private record() {
    const o = this.snapshot?.output;
    if (!o) return;
    const t = performance.now() / 1000;
    const h = this.history;
    h.push([t, o.lx, o.ly, o.rx, o.ry, o.lt, o.rt]);
    while (h.length && t - h[0][0] > 5) h.shift();
    if (this.page === "live") this.historyTick++;
  }
  private poll = async () => {
    await listen<Snapshot>("snapshot", (e) => this.onSnapshot(e.payload));
    const s = await api.snapshot().catch(() => null);
    if (s) this.onSnapshot(s);
    setInterval(() => { if (this.page === "live" && !document.hidden) this.record(); }, 100);
  };

  /** Edit the active profile; applied to the engine at once and saved shortly after. */
  edit(mutate: (p: Profile) => void) {
    if (!this.profile) return;
    mutate(this.profile);
    clearTimeout(this.saveTimer);
    const snapshot = $state.snapshot(this.profile) as Profile;
    this.saveTimer = setTimeout(() => {
      api.saveProfile(snapshot).catch((e) => this.toast(String(e), { error: true }));
    }, 150);
  }

  async run(op: () => Promise<Profile>) {
    try {
      this.profile = await op();
      this.profiles = await api.profiles();
    } catch (e) {
      this.toast(String(e), { error: true });
    }
  }

  toast(message: string, opts: { action?: Toast["action"]; error?: boolean; ms?: number } = {}) {
    const id = ++this.toastId;
    this.toasts.push({ id, message, action: opts.action, error: opts.error });
    setTimeout(() => this.dismiss(id), opts.ms ?? (opts.action ? 9000 : 5000));
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const app = new AppState();

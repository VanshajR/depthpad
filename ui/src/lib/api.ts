// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export type Source = "stream" | "held" | "release" | null;

export interface Report { buttons: number; lt: number; rt: number; lx: number; ly: number; rx: number; ry: number }

/** A keyboard model (data/devices/*.json): identity, key travel and how to draw it. */
export interface Device {
  id: string;
  name: string;
  vendor: string;
  driver: string;
  status: "tested" | "community" | "untested";
  travel_mm: number;
  width_units: number;
  height_units: number;
  /** Every key's rectangle, in key units from the top left. */
  layout: KeyRect[];
}
export interface KeyRect { pos: number; legend: string; x: number; y: number; w: number; h: number }

export interface Snapshot {
  state: "searching" | "streaming" | "noack" | "silent";
  error: string | null;
  firmware: string | null;
  rate: number;
  keys: { pos: number; depth: number; source: Source }[];
  output: Report;
  paused: boolean;
  blocking: boolean;
  pad: boolean;
  pad_error: string | null;
  pad_kind: PadKind;
  pad_parked: boolean;
  contention: boolean;
  last_event: string | null;
}

export type CurveMode = "analog" | "instant" | "turbo";
export interface Curve {
  deadzone_mm: number;
  full_mm: number;
  shape: number;
  press_mm: number;
  /** A drawn curve: 4 points of [travel mm, output 0..1]; replaces dead zone, full and shape. */
  points?: [number, number][] | null;
  anti_deadzone?: number;
  mode?: CurveMode;
}
/** The curve numbers a key can override. */
export type CurveKey = "deadzone_mm" | "full_mm" | "shape" | "press_mm";
export interface Binding extends Partial<Pick<Curve, CurveKey>> { key: string; target: string }
export interface Settings {
  stick_shape: "square" | "circle";
  last_key_priority: boolean;
  last_key_priority_sensitivity: number;
  snappy_joystick: boolean;
  block_keys: boolean;
  block_allow_alt_tab: boolean;
  pause_hotkey: string;
  block_all_keys: boolean;
  /** Depth assumed for a key that is down but sends no analog data. */
  unread_depth: "actuation" | "full";
  /** Where an even diagonal points, degrees from horizontal (45 = even). */
  diagonal_angle: number;
}
export interface Profile { name: string; settings: Settings; curve: Curve; bindings: Binding[] }
export interface KeyEntry { pos: number; key: string; hid: number }
export interface Prefs {
  profile: string;
  close_to_tray: boolean;
  start_minimized: boolean;
  check_updates: boolean;
  /** Hotkeys as key-name combos ("ControlRight+KeyP"); "" = none. */
  pause_hotkey: string | null;
  next_profile_hotkey: string;
  profile_hotkeys: Record<string, string>;
  /** How hotkey switches show outside the window. */
  indicator: "both" | "badge" | "sound" | "off";
  controller: PadKind;
  unplug_when_paused: boolean;
  /** Program file name ("gta5.exe") -> profile used while it's in front. */
  app_links: Record<string, string>;
  theme: Theme;
  accent: string;
}
export type Theme = "dark" | "midnight" | "oled" | "light";
export const THEMES: { id: Theme; label: string }[] = [
  { id: "dark", label: "Dark" }, { id: "midnight", label: "Midnight" }, { id: "oled", label: "OLED black" }, { id: "light", label: "Light" },
];
export const ACCENTS: { color: string; label: string }[] = [
  { color: "#4c8dff", label: "Blue" }, { color: "#8b6cff", label: "Violet" }, { color: "#14b8a6", label: "Teal" },
  { color: "#ec4899", label: "Pink" }, { color: "#e2e8f0", label: "Silver" },
];

/** Apply a theme and accent to the page now, and remember them for the next start (so the
 *  window opens in the right colours before the settings have loaded). */
export function applyAppearance(theme: Theme, accent: string) {
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.style.setProperty("--accent", accent);
  try {
    localStorage.setItem("depthpad.appearance", JSON.stringify({ theme, accent }));
  } catch {
    /* storage unavailable: settings still apply them on load */
  }
}

/** The remembered appearance, applied before anything renders. */
export function restoreAppearance() {
  try {
    const a = JSON.parse(localStorage.getItem("depthpad.appearance") ?? "null");
    if (a?.theme && a?.accent) applyAppearance(a.theme, a.accent);
  } catch {
    /* nothing remembered yet */
  }
}
export type PadKind = "xbox" | "ds4";
export interface RunningApp { exe: string; title: string }
export interface KeyboardEntry {
  id: string;
  name: string;
  vendor: string;
  status: "tested" | "community" | "untested";
  connected: boolean;
  note?: string | null;
}
/** Supported keyboards that are connected (plus the one in use), and which is the default. */
export interface Keyboards { list: KeyboardEntry[]; active: string; default: string | null; needs_choice: boolean }

export interface AppInfo { version: string; profile_dir: string; log_dir: string }

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  keys: () => invoke<KeyEntry[]>("keys"),
  snapshot: () => invoke<Snapshot | null>("snapshot"),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  profiles: () => invoke<string[]>("profiles"),
  activeProfile: () => invoke<Profile>("active_profile"),
  saveProfile: (profile: Profile) => invoke<void>("save_profile", { profile }),
  activateProfile: (name: string) => invoke<Profile>("activate_profile", { name }),
  newProfile: (fromActive: boolean) => invoke<Profile>("new_profile", { fromActive }),
  renameProfile: (name: string) => invoke<Profile>("rename_profile", { name }),
  deleteProfile: () => invoke<Profile>("delete_profile"),
  importProfile: (path: string) => invoke<Profile>("import_profile", { path }),
  exportProfile: (path: string) => invoke<void>("export_profile", { path }),
  builtinLayout: (name: string) => invoke<Profile>("builtin_layout", { name }),
  prefs: () => invoke<Prefs>("prefs"),
  setPref: (key: string, value: boolean) => invoke<void>("set_pref", { key, value }),
  isRecording: () => invoke<boolean>("is_recording"),
  setHotkey: (which: "pause" | "next", combo: string) => invoke<void>("set_hotkey", { which, combo }),
  setProfileHotkey: (name: string, combo: string) => invoke<void>("set_profile_hotkey", { name, combo }),
  suspendHotkeys: (suspended: boolean) => invoke<void>("suspend_hotkeys", { suspended }),
  setIndicator: (mode: string) => invoke<void>("set_indicator", { mode }),
  runningApps: () => invoke<RunningApp[]>("running_apps"),
  setAppLink: (exe: string, profile: string) => invoke<void>("set_app_link", { exe, profile }),
  setController: (kind: PadKind, unplugWhenPaused: boolean) => invoke<void>("set_controller", { kind, unplugWhenPaused }),
  setAppearance: (theme: Theme, accent: string) => invoke<void>("set_appearance", { theme, accent }),
  toggleRecording: () => invoke<[boolean, string | null]>("toggle_recording"),
  openFolder: (which: "logs" | "profiles") => invoke<void>("open_folder", { which }),
  vigembusInstalled: () => invoke<boolean>("vigembus_installed"),
  installVigembus: () => invoke<void>("install_vigembus"),
  createKeyboardReport: () => invoke<void>("create_keyboard_report"),
  device: () => invoke<Device>("device"),
  keyboards: () => invoke<Keyboards>("keyboards"),
  useKeyboard: (id: string, remember: boolean) => invoke<Keyboards>("use_keyboard", { id, remember }),
  setDefaultKeyboard: (id: string | null) => invoke<Keyboards>("set_default_keyboard", { id }),
};

export const CURVE_KEYS: CurveKey[] = ["deadzone_mm", "full_mm", "shape", "press_mm"];

// Mirrors Profile::curve_of in the core: a key with its own numbers doesn't use the drawn points.
export function curveOf(p: Profile, b: Binding | undefined): Curve {
  const d = p.curve;
  const ownShape = b?.deadzone_mm !== undefined || b?.full_mm !== undefined || b?.shape !== undefined;
  return {
    ...d,
    deadzone_mm: b?.deadzone_mm ?? d.deadzone_mm,
    full_mm: b?.full_mm ?? d.full_mm,
    shape: b?.shape ?? d.shape,
    press_mm: b?.press_mm ?? d.press_mm,
    points: ownShape ? null : d.points ?? null,
  };
}

/** Output for a key at `depth` (0..1) of a stroke `travel` mm long. */
/** Piecewise-linear output along drawn points (0 before the first). Mirrors the core. */
function along(p: [number, number][], mm: number): number {
  if (mm <= p[0][0]) return 0;
  for (let i = 1; i < p.length; i++) {
    const [x0, y0] = p[i - 1], [x1, y1] = p[i];
    if (mm <= x1) return x1 - x0 < 1e-9 ? y1 : y0 + ((y1 - y0) * (mm - x0)) / (x1 - x0);
  }
  return p[p.length - 1][1];
}

/** Output for a key at `depth` (0..1) of a stroke `travel` mm long. Mirrors Curve::apply. */
export function applyCurve(depth: number, c: Curve, travel: number): number {
  const mm = depth * travel;
  let v: number;
  if (c.points) {
    v = Math.min(1, Math.max(0, along(c.points, mm)));
  } else {
    const full = Math.max(c.full_mm, c.deadzone_mm + 0.05);
    const t = Math.min(1, Math.max(0, (mm - c.deadzone_mm) / (full - c.deadzone_mm)));
    v = t > 0 ? Math.pow(t, c.shape) : 0;
  }
  if (v <= 0 && !(c.points && mm > c.points[0][0])) return 0;
  if (c.mode === "turbo") return 1;
  const a = Math.min(0.9, Math.max(0, c.anti_deadzone ?? 0));
  return a + (1 - a) * v;
}

export const PRESETS = ["linear", "aggro", "slow", "smooth", "steep", "instant", "turbo"] as const;
export type Preset = (typeof PRESETS)[number];

/** A preset applied to a curve, keeping its dead zone and full output. Mirrors Curve::preset. */
export function presetCurve(c: Curve, name: Preset): Curve {
  const dz = c.deadzone_mm, full = Math.max(c.full_mm, dz + 0.2);
  const at = (f: number) => dz + (full - dz) * f;
  const shapes: Record<Preset, [[number, number], [number, number]]> = {
    linear: [[1 / 3, 1 / 3], [2 / 3, 2 / 3]], instant: [[1 / 3, 1 / 3], [2 / 3, 2 / 3]], turbo: [[1 / 3, 1 / 3], [2 / 3, 2 / 3]],
    aggro: [[0.25, 0.6], [0.6, 0.9]], slow: [[0.45, 0.15], [0.8, 0.45]], smooth: [[0.3, 0.1], [0.7, 0.9]], steep: [[0.4, 0.05], [0.6, 0.95]],
  };
  const [[x1, y1], [x2, y2]] = shapes[name];
  return {
    ...c,
    points: [[dz, 0], [at(x1), y1], [at(x2), y2], [full, 1]],
    mode: name === "instant" ? "instant" : name === "turbo" ? "turbo" : "analog",
  };
}

/** The curve as 4 points, for editing a curve that doesn't have any yet. */
export function pointsOf(c: Curve): [number, number][] {
  if (c.points) return c.points.map((p) => [p[0], p[1]] as [number, number]);
  const dz = c.deadzone_mm, full = Math.max(c.full_mm, dz + 0.2);
  return [0, 1 / 3, 2 / 3, 1].map((f) => [dz + (full - dz) * f, f === 0 ? 0 : Math.pow(f, c.shape)] as [number, number]);
}


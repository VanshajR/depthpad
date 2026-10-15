// Development-only preview: the real UI with Tauri's IPC mocked by realistic data.
// http://localhost:1420/preview.html?page=mapping&key=34&view=1
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { mount } from "svelte";
import ak680 from "../../crates/depthpad-core/data/devices/ajazz-ak680-v2-he.json";

// ?device=<model id> previews another keyboard's drawing
const allDevices = import.meta.glob("../../crates/depthpad-core/data/devices/*.json", { eager: true, import: "default" }) as Record<string, typeof ak680>;
const wanted = new URLSearchParams(location.search).get("device");
const device = (Object.values(allDevices).find((d) => d.id === wanted) ?? ak680) as typeof ak680;
import App from "./App.svelte";
import "./lib/tokens.css";
import { app } from "./lib/state.svelte";

const q = new URLSearchParams(location.search);
const keys = Object.entries(device.keys as Record<string, { hid: number; key: string }>).map(([pos, v]) => ({ pos: +pos, ...v }));
const b = (key: string, target: string) => ({ key, target });
const profile = {
  name: "Default",
  settings: { stick_shape: "square", last_key_priority: false, last_key_priority_sensitivity: 0.02, snappy_joystick: false,
              block_keys: false, block_allow_alt_tab: true, pause_hotkey: "ControlRight+KeyP",
              unread_depth: "actuation", diagonal_angle: 45 },
  curve: { deadzone_mm: 0.2, full_mm: 2.6, shape: 1.0, press_mm: 1.0, points: null, anti_deadzone: 0, mode: "analog" },
  bindings: [b("KeyW", "LS_UP"), b("KeyS", "LS_DOWN"), b("KeyA", "LS_LEFT"), b("KeyD", "LS_RIGHT"),
             b("ShiftLeft", "LT"), b("KeyF", "RT"), b("ArrowUp", "RS_UP"), b("ArrowDown", "RS_DOWN"),
             b("ArrowLeft", "RS_LEFT"), b("ArrowRight", "RS_RIGHT"), b("Space", "A"), b("KeyC", "B"),
             b("KeyR", "X"), b("KeyG", "Y"), b("KeyQ", "LB"), b("KeyE", "RB"), b("Tab", "BACK"), b("Enter", "START")],
};

function snapshot() {
  const t = performance.now() / 1000;
  const w = 0.5 + 0.45 * Math.sin(t * 2.2);
  const sh = 0.45 + 0.4 * Math.sin(t * 1.3 + 1);
  return {
    state: "streaming", error: null, firmware: "11.01", rate: 990, paused: false, blocking: false, pad: true,
    pad_error: null, pad_kind: "xbox", pad_parked: false, contention: false, last_event: "keyboard connected (firmware 11.01)",
    keys: [{ pos: 34, depth: w, source: "stream" }, { pos: 51, depth: 1, source: "held" }, { pos: 64, depth: sh, source: "stream" }],
    output: { buttons: 0x1000, lt: Math.max(0, (sh * 3.4 - 0.2) / 2.4), rt: 0, lx: 1, ly: Math.min(1, Math.max(0, (w * 3.4 - 0.2) / 2.4)), rx: 0, ry: 0 },
  };
}

// Same placement as device::place in the core (KLE conventions).
type RowKey = [number, string, number] | { pos: number; legend?: string; w?: number; h?: number; x?: number; y?: number };
function placed(rows: RowKey[][]) {
  const out: { pos: number; legend: string; x: number; y: number; w: number; h: number }[] = [];
  let y = 0;
  for (const row of rows) {
    let x = 0;
    for (const k of row) {
      const o = Array.isArray(k) ? { pos: k[0], legend: k[1], w: k[2] } : k;
      y += o.y ?? 0;
      x += o.x ?? 0;
      out.push({ pos: o.pos, legend: o.legend ?? "", x, y, w: o.w ?? 1, h: o.h ?? 1 });
      x += o.w ?? 1;
    }
    y += 1;
  }
  return out;
}
const layout = placed(device.rows as RowKey[][]);
const deviceView = { ...device, layout, height_units: Math.max(...layout.map((k) => k.y + k.h)) };

function keyboards() {
  const ak = { id: device.id, name: device.name, vendor: device.vendor, status: "tested", connected: true };
  const two = q.get("kbs") === "2";
  const other = { id: "nuphy-halo65-he", name: "NuPhy Halo65 HE", vendor: "NuPhy", status: "untested", connected: true };
  return { list: two ? [ak, other] : [ak], active: device.id, default: null, needs_choice: two && q.get("choose") === "1" };
}

mockWindows("main");
mockIPC((cmd) => {
  switch (cmd) {
    case "app_info": return { version: "0.1.0", profile_dir: "C:\\Users\\you\\AppData\\Roaming\\Depthpad\\profiles", log_dir: "C:\\Users\\you\\AppData\\Local\\Depthpad\\logs" };
    case "keys": return keys;
    case "device": return deviceView;
    case "keyboards": return keyboards();
    case "running_apps": return [{ exe: "gta5.exe", title: "Grand Theft Auto V" }, { exe: "chrome.exe", title: "Google Chrome" }];
    case "use_keyboard": case "set_default_keyboard": return { ...keyboards(), needs_choice: false };
    case "snapshot": return snapshot();
    case "active_profile": return profile;
    case "profiles": return ["Default", "Racing"];
    case "prefs": return { profile: "Default", close_to_tray: true, start_minimized: false, check_updates: true,
                           pause_hotkey: "ControlRight+KeyP", next_profile_hotkey: "ControlRight+KeyO",
                           profile_hotkeys: { Racing: "ControlRight+Digit2" }, indicator: "both",
                           controller: (q.get("pad") === "ds4" ? "ds4" : "xbox"), unplug_when_paused: false, app_links: { "gta5.exe": "Racing" },
                           theme: (q.get("theme") ?? "dark"), accent: (q.get("accent") ?? "#4c8dff") };
    case "vigembus_installed": return true;
    case "is_recording": return q.get("rec") === "1";
    case "plugin:autostart|is_enabled": return false;
    default: return null;
  }
}, { shouldMockEvents: true });
setInterval(() => emit("snapshot", snapshot()), 16);

if (q.get("view")) localStorage.setItem("depthpad.liveView", q.get("view")!);
const page = q.get("page");
if (page) app.page = page as typeof app.page;
if (q.get("key")) app.mapKey = Number(q.get("key"));
mount(App, { target: document.getElementById("app")! });

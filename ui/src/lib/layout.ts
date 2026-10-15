// Controller-input labels and helpers. Keyboard geometry comes from the device (api.device()).

export const TARGET_LONG: Record<string, string> = {
  LS_UP: "Left stick up", LS_DOWN: "Left stick down", LS_LEFT: "Left stick left", LS_RIGHT: "Left stick right",
  RS_UP: "Right stick up", RS_DOWN: "Right stick down", RS_LEFT: "Right stick left", RS_RIGHT: "Right stick right",
  LT: "Left trigger", RT: "Right trigger", A: "A button", B: "B button", X: "X button", Y: "Y button",
  LB: "Left bumper", RB: "Right bumper", BACK: "View button", START: "Menu button", GUIDE: "Guide button",
  LS: "Left stick click", RS: "Right stick click", DPAD_UP: "D-pad up", DPAD_DOWN: "D-pad down",
  DPAD_LEFT: "D-pad left", DPAD_RIGHT: "D-pad right",
};

export const TARGET_GROUPS: [string, string[]][] = [
  ["Left stick", ["LS_UP", "LS_DOWN", "LS_LEFT", "LS_RIGHT"]],
  ["Right stick", ["RS_UP", "RS_DOWN", "RS_LEFT", "RS_RIGHT"]],
  ["Triggers", ["LT", "RT"]],
  ["Face", ["A", "B", "X", "Y"]],
  ["Bumpers", ["LB", "RB"]],
  ["System", ["BACK", "START", "GUIDE"]],
  ["Clicks", ["LS", "RS"]],
  ["D-pad", ["DPAD_UP", "DPAD_DOWN", "DPAD_LEFT", "DPAD_RIGHT"]],
];

export const ANALOG = new Set(["LS_UP", "LS_DOWN", "LS_LEFT", "LS_RIGHT", "RS_UP", "RS_DOWN", "RS_LEFT", "RS_RIGHT", "LT", "RT"]);

export const FACE: Record<string, string> = { A: "#2fbf71", B: "#ef5350", X: "#4c8dff", Y: "#e6b422" };

/** PlayStation (DualShock 4) equivalents: face symbols and colours, and input names. */
export const PS_FACE: Record<string, { symbol: "cross" | "circle" | "square" | "triangle"; color: string }> = {
  A: { symbol: "cross", color: "#7aa8ff" }, B: { symbol: "circle", color: "#ff6b7a" },
  X: { symbol: "square", color: "#e98fd0" }, Y: { symbol: "triangle", color: "#2fd3a9" },
};
export const PS_LONG: Record<string, string> = {
  A: "Cross", B: "Circle", X: "Square", Y: "Triangle", LB: "L1", RB: "R1", LT: "L2 trigger", RT: "R2 trigger",
  BACK: "Share", START: "Options", GUIDE: "PS button", LS: "L3 (left stick click)", RS: "R3 (right stick click)",
};
export const PS_SHORT: Record<string, string> = { LB: "L1", RB: "R1", LT: "L2", RT: "R2" };

export const BUTTON_BITS: Record<string, number> = {
  DPAD_UP: 0x0001, DPAD_DOWN: 0x0002, DPAD_LEFT: 0x0004, DPAD_RIGHT: 0x0008, START: 0x0010, BACK: 0x0020,
  LS: 0x0040, RS: 0x0080, LB: 0x0100, RB: 0x0200, GUIDE: 0x0400, A: 0x1000, B: 0x2000, X: 0x4000, Y: 0x8000,
};

export const KEY_NAMES: Record<string, string> = {
  ControlRight: "Right Ctrl", ControlLeft: "Left Ctrl", ShiftLeft: "Left Shift", ShiftRight: "Right Shift",
  AltLeft: "Left Alt", AltRight: "Right Alt", MetaLeft: "Win",
};

export function prettyHotkey(v: string): string {
  if (!v) return "None";
  return v.split("+").map((p) => KEY_NAMES[p] ?? p.replace(/^Key/, "").replace(/^Digit/, "")).join(" + ");
}

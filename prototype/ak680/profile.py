"""Profiles: which keys drive which controller inputs, with per-binding curves.

A profile is a JSON file:
  {"name": ..., "settings": {...}, "curve": {defaults}, "bindings": [{"key", "target", optional curve}]}

Several keys may drive one target; the deepest one wins (HallJoy's ReadSide).
"""
import copy
import json
import os
import re

STICK_TARGETS = {
    "LS_UP": ("Left stick", "up"), "LS_DOWN": ("Left stick", "down"),
    "LS_LEFT": ("Left stick", "left"), "LS_RIGHT": ("Left stick", "right"),
    "RS_UP": ("Right stick", "up"), "RS_DOWN": ("Right stick", "down"),
    "RS_LEFT": ("Right stick", "left"), "RS_RIGHT": ("Right stick", "right"),
}
TRIGGER_TARGETS = {"LT": "Left trigger", "RT": "Right trigger"}
BUTTON_TARGETS = {
    "A": "A", "B": "B", "X": "X", "Y": "Y", "LB": "Left bumper", "RB": "Right bumper",
    "BACK": "View / Back", "START": "Menu / Start", "LS": "Left stick click", "RS": "Right stick click",
    "DPAD_UP": "D-pad up", "DPAD_DOWN": "D-pad down", "DPAD_LEFT": "D-pad left", "DPAD_RIGHT": "D-pad right",
    "GUIDE": "Guide",
}
TARGETS = {**{k: f"{a} {d}" for k, (a, d) in STICK_TARGETS.items()}, **TRIGGER_TARGETS, **BUTTON_TARGETS}
ANALOG_TARGETS = set(STICK_TARGETS) | set(TRIGGER_TARGETS)

CURVE_DEFAULTS = {"deadzone_mm": 0.2, "full_mm": 2.6, "shape": 1.0, "press_mm": 1.0}
SETTINGS_DEFAULTS = {
    "stick_shape": "square",                 # "square" (HallJoy) or "circle"
    "last_key_priority": False,
    "last_key_priority_sensitivity": 0.02,
    "snappy_joystick": False,
    "block_keys": False,
    "block_allow_alt_tab": True,
    "pause_hotkey": "ControlRight+KeyP",
}
MAX_TRAVEL_MM = 3.4


def _b(key, target, **curve):
    return {"key": key, "target": target, **curve}


BUILTIN = {
    "Default": {
        "name": "Default",
        "settings": dict(SETTINGS_DEFAULTS),
        "curve": dict(CURVE_DEFAULTS),
        "bindings": [
            _b("KeyW", "LS_UP"), _b("KeyS", "LS_DOWN"), _b("KeyA", "LS_LEFT"), _b("KeyD", "LS_RIGHT"),
            _b("ArrowUp", "RS_UP"), _b("ArrowDown", "RS_DOWN"), _b("ArrowLeft", "RS_LEFT"), _b("ArrowRight", "RS_RIGHT"),
            _b("ShiftLeft", "LT"), _b("KeyF", "RT"),
            _b("Space", "A"), _b("KeyC", "B"), _b("KeyR", "X"), _b("KeyG", "Y"),
            _b("KeyQ", "LB"), _b("KeyE", "RB"), _b("Tab", "BACK"), _b("Enter", "START"),
        ],
    },
    "Racing": {
        "name": "Racing",
        "settings": {**SETTINGS_DEFAULTS, "block_keys": True},
        "curve": dict(CURVE_DEFAULTS),
        "bindings": [
            _b("KeyW", "RT"), _b("KeyS", "LT"), _b("KeyA", "LS_LEFT"), _b("KeyD", "LS_RIGHT"),
            _b("ArrowUp", "RS_UP"), _b("ArrowDown", "RS_DOWN"), _b("ArrowLeft", "RS_LEFT"), _b("ArrowRight", "RS_RIGHT"),
            _b("Space", "A"), _b("KeyC", "B"), _b("KeyR", "X"), _b("KeyG", "Y"),
            _b("KeyQ", "LB"), _b("KeyE", "RB"), _b("Tab", "BACK"), _b("Enter", "START"),
        ],
    },
}


def curve_of(profile, binding):
    c = dict(CURVE_DEFAULTS)
    c.update({k: v for k, v in profile.get("curve", {}).items() if k in CURVE_DEFAULTS})
    c.update({k: v for k, v in binding.items() if k in CURVE_DEFAULTS})
    return c


def apply_curve(depth, curve):
    """depth: 0..1 of full travel -> 0..1 output."""
    mm = depth * MAX_TRAVEL_MM
    dz, full = curve["deadzone_mm"], max(curve["full_mm"], curve["deadzone_mm"] + 0.05)
    v = min(1.0, max(0.0, (mm - dz) / (full - dz)))
    return v ** curve["shape"] if v > 0 else 0.0


def validate(profile, key_names):
    """List of problems; empty when the profile can be used."""
    problems = []
    for b in profile.get("bindings", []):
        if b.get("key") not in key_names:
            problems.append(f"unknown key {b.get('key')!r}")
        if b.get("target") not in TARGETS:
            problems.append(f"unknown target {b.get('target')!r}")
        c = curve_of(profile, b)
        if not 0 <= c["deadzone_mm"] < c["full_mm"] <= MAX_TRAVEL_MM:
            problems.append(f"{b.get('key')}: deadzone must be below full press, within 0-{MAX_TRAVEL_MM} mm")
        if not 0.2 <= c["shape"] <= 5:
            problems.append(f"{b.get('key')}: shape must be 0.2-5")
    hk = profile.get("settings", {}).get("pause_hotkey") or ""
    for part in filter(None, hk.split("+")):
        if part not in key_names:
            problems.append(f"pause hotkey: unknown key {part!r}")
    return problems


def from_legacy(cfg, name="Imported"):
    """Convert the first config.json format (sticks/triggers/buttons sections)."""
    p = {"name": name, "settings": {k: cfg[k] for k in SETTINGS_DEFAULTS if k in cfg},
         "curve": {k: cfg[k] for k in ("deadzone_mm", "full_mm") if k in cfg}, "bindings": []}
    if "button_mm" in cfg:
        p["curve"]["press_mm"] = cfg["button_mm"]
    for stick, prefix in (("left_stick", "LS"), ("right_stick", "RS")):
        for d, key in (cfg.get(stick) or {}).items():
            p["bindings"].append(_b(key, f"{prefix}_{d.upper()}"))
    for trig, t in (("left_trigger", "LT"), ("right_trigger", "RT")):
        if cfg.get(trig):
            p["bindings"].append(_b(cfg[trig], t))
    for btn, key in (cfg.get("buttons") or {}).items():
        p["bindings"].append(_b(key, btn))
    return p


class ProfileStore:
    """Profiles as JSON files in a folder; built-ins are written on first run."""

    def __init__(self, folder):
        self.folder = folder
        os.makedirs(folder, exist_ok=True)
        if not any(f.endswith(".json") for f in os.listdir(folder)):
            for p in BUILTIN.values():
                self.save(copy.deepcopy(p))

    def _path(self, name):
        safe = re.sub(r'[<>:"/\\|?*\x00-\x1f]', "_", name).strip() or "Profile"
        return os.path.join(self.folder, safe + ".json")

    def names(self):
        out = []
        for f in sorted(os.listdir(self.folder)):
            if f.endswith(".json"):
                try:
                    out.append(self.load_file(os.path.join(self.folder, f))["name"])
                except (OSError, ValueError, KeyError):
                    pass
        return out

    @staticmethod
    def load_file(path):
        with open(path, encoding="utf8") as f:
            p = json.load(f)
        if "bindings" not in p:
            p = from_legacy(p, os.path.splitext(os.path.basename(path))[0])
        p.setdefault("name", os.path.splitext(os.path.basename(path))[0])
        p["settings"] = {**SETTINGS_DEFAULTS, **p.get("settings", {})}
        p["curve"] = {**CURVE_DEFAULTS, **p.get("curve", {})}
        return p

    def load(self, name):
        return self.load_file(self._path(name))

    def save(self, profile):
        tmp = self._path(profile["name"]) + ".tmp"
        with open(tmp, "w", encoding="utf8", newline="\n") as f:
            json.dump(profile, f, indent=2)
        os.replace(tmp, self._path(profile["name"]))

    def delete(self, name):
        try:
            os.remove(self._path(name))
        except FileNotFoundError:
            pass

    def unique_name(self, base):
        names = set(self.names())
        if base not in names:
            return base
        i = 2
        while f"{base} {i}" in names:
            i += 1
        return f"{base} {i}"

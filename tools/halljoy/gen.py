"""Depthpad keyboard families and variants from HallJoy's layouts (hj_layouts.json).

    python -I gen.py            regenerate every family in families.py

Writes crates/depthpad-core/data/devices/<family>.json (the family: USB match, driver, a typical
layout), data/variants/<family>.json (each board: name, firmware names, layout) and
data/variants/index.rs, and adds new families to BUNDLED in device.rs.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import families  # noqa: E402

ROOT = CORE
LAYOUTS = json.load(open(os.path.join(HERE, "hj_layouts.json"), encoding="utf8"))

# HID usage -> KeyboardEvent.code-style name
NAME = {
    0x04: "KeyA", 0x05: "KeyB", 0x06: "KeyC", 0x07: "KeyD", 0x08: "KeyE", 0x09: "KeyF", 0x0A: "KeyG", 0x0B: "KeyH",
    0x0C: "KeyI", 0x0D: "KeyJ", 0x0E: "KeyK", 0x0F: "KeyL", 0x10: "KeyM", 0x11: "KeyN", 0x12: "KeyO", 0x13: "KeyP",
    0x14: "KeyQ", 0x15: "KeyR", 0x16: "KeyS", 0x17: "KeyT", 0x18: "KeyU", 0x19: "KeyV", 0x1A: "KeyW", 0x1B: "KeyX",
    0x1C: "KeyY", 0x1D: "KeyZ", 0x1E: "Digit1", 0x1F: "Digit2", 0x20: "Digit3", 0x21: "Digit4", 0x22: "Digit5",
    0x23: "Digit6", 0x24: "Digit7", 0x25: "Digit8", 0x26: "Digit9", 0x27: "Digit0", 0x28: "Enter", 0x29: "Escape",
    0x2A: "Backspace", 0x2B: "Tab", 0x2C: "Space", 0x2D: "Minus", 0x2E: "Equal", 0x2F: "BracketLeft",
    0x30: "BracketRight", 0x31: "Backslash", 0x32: "IntlHash", 0x33: "Semicolon", 0x34: "Quote", 0x35: "Backquote",
    0x36: "Comma", 0x37: "Period", 0x38: "Slash", 0x39: "CapsLock",
    **{0x3A + i: f"F{i + 1}" for i in range(12)},
    0x46: "PrintScreen", 0x47: "ScrollLock", 0x48: "Pause", 0x49: "Insert", 0x4A: "Home", 0x4B: "PageUp",
    0x4C: "Delete", 0x4D: "End", 0x4E: "PageDown", 0x4F: "ArrowRight", 0x50: "ArrowLeft", 0x51: "ArrowDown",
    0x52: "ArrowUp", 0x53: "NumLock", 0x54: "NumpadDivide", 0x55: "NumpadMultiply", 0x56: "NumpadSubtract",
    0x57: "NumpadAdd", 0x58: "NumpadEnter", **{0x59 + i: f"Numpad{i + 1}" for i in range(9)}, 0x62: "Numpad0",
    0x63: "NumpadDecimal", 0x64: "IntlBackslash", 0x65: "ContextMenu", 0x66: "Power", 0x67: "NumpadEqual",
    **{0x68 + i: f"F{13 + i}" for i in range(12)},
    0x7F: "AudioVolumeMute", 0x80: "AudioVolumeUp", 0x81: "AudioVolumeDown", 0x85: "NumpadComma",
    0x87: "IntlRo", 0x88: "KanaMode", 0x89: "IntlYen", 0x8A: "Convert", 0x8B: "NonConvert", 0x90: "Lang1", 0x91: "Lang2",
    0xE0: "ControlLeft", 0xE1: "ShiftLeft", 0xE2: "AltLeft", 0xE3: "MetaLeft", 0xE4: "ControlRight",
    0xE5: "ShiftRight", 0xE6: "AltRight", 0xE7: "MetaRight",
}
LEG = {
    "Escape": "Esc", "Backquote": "`", "Minus": "-", "Equal": "=", "Insert": "Ins", "PageUp": "PgUp", "Delete": "Del",
    "PageDown": "PgDn", "BracketLeft": "[", "BracketRight": "]", "Backslash": "\\", "IntlHash": "#", "CapsLock": "Caps",
    "Semicolon": ";", "Quote": "'", "ShiftLeft": "Shift", "Comma": ",", "Period": ".", "Slash": "/", "ShiftRight": "Shift",
    "ControlLeft": "Ctrl", "MetaLeft": "Win", "AltLeft": "Alt", "Space": "", "AltRight": "Alt", "MetaRight": "Win",
    "ContextMenu": "Menu", "ControlRight": "Ctrl", "ArrowUp": "↑", "ArrowLeft": "←", "ArrowDown": "↓", "ArrowRight": "→",
    "PrintScreen": "PrtSc", "ScrollLock": "ScrLk", "NumLock": "Num", "NumpadDivide": "/", "NumpadMultiply": "*",
    "NumpadSubtract": "-", "NumpadAdd": "+", "NumpadEnter": "Enter", "NumpadDecimal": ".", "IntlBackslash": "\\",
    "IntlRo": "\\", "IntlYen": "¥", "AudioVolumeMute": "Mute", "AudioVolumeUp": "Vol+", "AudioVolumeDown": "Vol-",
}


def legend(name, label):
    if name in LEG:
        return LEG[name]
    for p in ("Key", "Digit", "Numpad"):
        if name.startswith(p):
            return name[len(p):]
    return name if name in NAME.values() else label


def camel(label):
    words = re.findall(r"[A-Za-z0-9]+", label)
    return "".join(w[:1].upper() + w[1:] for w in words) or "Key"


def matrix_layout(codes):
    """A drawing from a key matrix: {(row, column): code}, rows top to bottom, keys left to right,
    with the usual widths for their names (for boards HallJoy has no drawing of)."""
    names = {NAME.get(c) for c in codes.values()}
    iso = "IntlHash" in names  # the # key sits where an ANSI Enter's left part would
    widths = {"Backspace": 2, "Tab": 1.5, "CapsLock": 1.75, "Enter": 1.25 if iso else 2.25, "Space": 6.25,
              "ShiftLeft": 1.25 if "IntlBackslash" in names else 2.25}
    keys = []
    for y, row in enumerate(sorted({r for r, _ in codes})):
        cells = [codes[k] for k in sorted(k for k in codes if k[0] == row)]
        row_names = [NAME.get(c, "") for c in cells]
        arrows = "ArrowUp" in row_names or "ArrowLeft" in row_names
        x = 0.0
        for c in cells:
            n = NAME.get(c, "")
            w = widths.get(n, 1)
            if n == "Backslash" and row == 2:
                w = 1.5
            if n == "ShiftRight":
                w = 1.75 if arrows else 2.75
            if row == max(r for r, _ in codes) and n in ("ControlLeft", "MetaLeft", "AltLeft"):
                w = 1.25
            if row == max(r for r, _ in codes) and n in ("AltRight", "MetaRight", "ContextMenu", "ControlRight") or c == 0x409:
                w = 1 if arrows else 1.25
            label = "Fn" if c == 0x409 else n
            keys.append({"label": label, "hid": c, "x": x, "y": float(y), "w": w, "h": 1.0})
            x += w
    return keys


def convert(preset):
    """A HallJoy layout (a preset name, or keys from `matrix_layout`) -> (width_units, rows, keys)
    in Depthpad's format."""
    src = LAYOUTS[preset]["keys"] if isinstance(preset, str) else preset
    used_pos, used_names, used_codes = set(), set(), set()
    keys, placed = {}, []
    # HID keys keep their usage as position; others get free bytes
    for k in src:
        if 0 < k["hid"] < 0xE8:
            used_pos.add(k["hid"])
    free = [p for p in list(range(0xE8, 0x100)) + list(range(0xA5, 0xE0)) + list(range(1, 4)) if p not in used_pos]
    for k in src:
        code = k["hid"]
        if code in used_codes:
            code = 0  # a second key with the same code: drawn, not reported
        if code:
            used_codes.add(code)
        if 0 < code < 0xE8 and code in NAME:
            pos, hid, name = code, code, NAME[code]
            if name == "IntlHash" and not any(x["hid"] == 0x31 for x in src):
                name = "Backslash"  # an ISO board's # key, as browsers name it
        else:
            pos, hid = free.pop(0), 0
            name = "Fn" if code in (0x409, 0xFA) or k["label"].lower() == "fn" else camel(k["label"])
        base, n = name, 2
        while name in used_names:
            name, n = f"{base}{n}", n + 1
        used_names.add(name)
        info = {"hid": hid, "key": name}
        if code and code != hid:
            info["code"] = code
        keys[str(pos)] = info
        placed.append((k, pos, legend(name, k["label"])))
    # rows: keys sharing a top edge, left to right
    rows, y_cursor = [], 0.0
    for y in sorted({k["y"] for k, _, _ in placed}):
        row, x_cursor = [], 0.0
        for k, pos, leg in sorted((p for p in placed if p[0]["y"] == y), key=lambda p: p[0]["x"]):
            e = {"pos": pos, "legend": leg, "w": k["w"]}
            if k["h"] != 1:
                e["h"] = k["h"]
            if k.get("notch"):
                e["notch"] = k["notch"]
            gap = round(k["x"] - x_cursor, 2)
            if gap:
                e["x"] = gap
            if not row and round(y - y_cursor, 2):
                e["y"] = round(y - y_cursor, 2)
            row.append([pos, leg, k["w"]] if set(e) == {"pos", "legend", "w"} else e)
            x_cursor = round(k["x"] + k["w"], 2)
        rows.append(row)
        y_cursor = round(y + 1, 2)
    width = round(max(k["x"] + k["w"] for k, _, _ in placed), 2)
    return width, rows, dict(sorted(keys.items(), key=lambda kv: int(kv[0])))


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def dump(obj):
    t = json.dumps(obj, indent=2, ensure_ascii=False)
    t = re.sub(r'\[\n\s+(\d+),\n\s+("(?:[^"\\]|\\.)*"),\n\s+([\d.]+)\n\s+\]', r"[\1, \2, \3]", t)
    t = re.sub(r'\{\n\s+("hid": \d+),\n\s+("key": "[^"]+")(?:,\n\s+("code": \d+))?\n\s+\}',
               lambda m: "{ " + ", ".join(g for g in m.groups() if g) + " }", t)
    t = re.sub(r'\{\n\s+("pos": [^\n]+)\n((?:\s+"[a-z]+": [^\n]+\n)*?)\s+\}',
               lambda m: "{ " + " ".join([m.group(1)] + [l.strip() for l in m.group(2).splitlines()]) + " }", t)
    return t + "\n"


def main():
    FAMILIES = families.FAMILIES
    index, files = [], []
    for fam in FAMILIES:
        width, rows, keys = convert(fam["layout"])
        model = {k: fam[k] for k in ("id", "name", "vendor", "driver") if k in fam}
        model.update({"status": "untested", "usb": fam["usb"], "travel_mm": fam.get("travel_mm", 3.5),
                      "width_units": width})
        if fam.get("note"):
            model["note"] = fam["note"]
        model.update({"rows": rows, "keys": keys})
        open(os.path.join(ROOT, "data", "devices", fam["id"] + ".json"), "w", encoding="utf8", newline="\n").write(dump(model))
        variants = []
        for v in fam.get("variants", []):
            width, rows, keys = convert(v["layout"])
            vid = v.get("id") or slug(v["name"])
            entry = {"id": vid, "name": v["name"], "vendor": v.get("vendor", fam["vendor"]), "identity": v["identity"],
                     "width_units": width}
            if v.get("travel_mm"):
                entry["travel_mm"] = v["travel_mm"]
            if v.get("note"):
                entry["note"] = v["note"]
            entry.update({"rows": rows, "keys": keys})
            variants.append(entry)
            index.append((vid, fam["id"], v["identity"]))
        if variants:
            # one board per line, compact: these files are embedded in the app
            body = ",\n".join(json.dumps(v, ensure_ascii=False, separators=(",", ":")) for v in variants)
            open(os.path.join(ROOT, "data", "variants", fam["id"] + ".json"), "w", encoding="utf8", newline="\n").write(
                '{"variants": [\n' + body + "\n]}\n")
            files.append(fam["id"])
        print(f'{fam["id"]}: {len(variants)} variants')
    rs = ["// Generated by the HallJoy layout tools from data/variants/*.json. Do not edit.",
          "pub const INDEX: &[(&str, &str, &[&str])] = &["]
    rs += [f"    ({json.dumps(v)}, {json.dumps(f)}, &[{', '.join(json.dumps(n) for n in ids)}])," for v, f, ids in index]
    rs += ["];", "pub const FILES: &[(&str, &str)] = &["]
    rs += [f'    ({json.dumps(f)}, include_str!("{f}.json")),' for f in files]
    rs += ["];", ""]
    open(os.path.join(ROOT, "data", "variants", "index.rs"), "w", encoding="utf8", newline="\n").write("\n".join(rs))
    p = os.path.join(ROOT, "src", "device.rs")
    s = open(p, encoding="utf8").read()
    for fam in FAMILIES:
        line = f'    include_str!("../data/devices/{fam["id"]}.json"),\n'
        if line not in s:
            s = s.replace("];\n\nfn hex<", line + "];\n\nfn hex<", 1)
    open(p, "w", encoding="utf8", newline="\n").write(s)


if __name__ == "__main__":
    main()

"""Extract every named HallJoy layout preset (KeyDef arrays, pixel geometry) into key units.

Output: hj_layouts.json = {preset name: {"array": g_name, "brand": ..., "keys": [{label, hid, x, y, w, h}]}}
Geometry: HallJoy draws keys at pixel x/y with a ~46 px pitch (key 40-42 px). Units are snapped to
quarters of the layout's own pitch.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import glob, json, os, re, statistics, sys

SRC = HALLJOY_SRC
files = [os.path.join(SRC, "keyboard_layout.cpp")] + glob.glob(os.path.join(SRC, "*.h")) + \
    glob.glob(os.path.join(SRC, "generated", "layout_pipeline", "*"))
text = {f: open(f, encoding="utf8", errors="replace").read() for f in files}
NAMED = {"halljoy::keycode::kFn": "1033", "halljoy::keycode::kOem1": "1027", "kFn": "1033", "kOem1": "1027"}
for f in text:
    for k, v in NAMED.items():
        text[f] = text[f].replace(k + ",", v + ",")

STR = r'L"((?:[^"\\]|\\.)*)"'
arrays = {}
for f, t in text.items():
    for m in re.finditer(r"KeyDef\s+(g_\w+)\s*\[\]\s*=\s*\{(.*?)\n\s*\};", t, re.S):
        keys = []
        for e in re.finditer(r"\{\s*" + STR + r"\s*,([^{}]*)\}", m.group(2)):
            label = bytes(e.group(1), "utf8").decode("unicode_escape")
            nums = [int(v) for v in re.findall(r"-?\d+", e.group(2))]
            hid, row, x, w = nums[:4]
            h = nums[4] if len(nums) > 4 else 40
            y = nums[5] if len(nums) > 5 else -1
            if y < 0:
                y = row * 46
            keys.append({"label": label, "hid": hid, "px": [x, y, w, h], "notch": nums[6:8]})
        if keys:
            arrays[m.group(1)] = keys

presets = {}
# presets registered under a named constant (halljoy::sharklayout::X65 = L"...")
consts = {}
for t in text.values():
    for m in re.finditer(r"namespace ([\w:]+)\s*\{(.*?)\n\}", t, re.S):
        ns = m.group(1) if m.group(1).startswith("halljoy::") else "halljoy::" + m.group(1)
        for c in re.finditer(r"constexpr wchar_t (\w+)\[\]\s*=\s*" + STR, m.group(2)):
            consts[f"{ns}::{c.group(1)}"] = c.group(2)
for t in text.values():
    for m in re.finditer(r"\{\s*(halljoy::\w+::\w+)\s*,\s*(g_\w+)\s*,\s*\(int\)[^,]+,\s*" + STR + r"\s*\}", t):
        if m.group(1) in consts:
            presets.setdefault(consts[m.group(1)], (m.group(2), m.group(3)))
for f, t in text.items():
    for m in re.finditer(r"\{\s*" + STR + r"\s*,\s*(g_\w+)\s*,\s*\(int\)[^,]+,\s*" + STR + r"\s*\}", t):
        presets.setdefault(m.group(1), (m.group(2), m.group(3)))


def units(keys):
    """Pixel geometry -> key units, snapped to 1/4."""
    # pitch and key gap: the pair that puts key edges closest to quarter units
    unit_w = statistics.median([k["px"][2] for k in keys])
    def err(pitch):
        gap = pitch - unit_w
        e = 0.0
        for k in keys:
            x, y, w, h = k["px"]
            for v in (x / pitch, (x + w + gap) / pitch, y / pitch):
                f = v * 4
                e += abs(f - round(f))
        return e
    pitch = min((unit_w + g / 20 for g in range(0, 300)), key=err)
    gap = pitch - unit_w
    def q(v, offset=0.0):
        u = v / pitch - offset
        quarter = round(u * 4) / 4
        return round((quarter if abs(u - quarter) <= 0.06 else u) + offset, 2)
    # rows below a gap (an F-row's) sit off the quarter grid by a common amount: snap to that
    rem = [round((k["px"][1] / pitch) % 0.25, 2) for k in keys]
    y_off = statistics.mode(rem) if rem else 0.0
    if y_off > 0.2:
        y_off -= 0.25
    if abs(y_off) < 0.03:
        y_off = 0.0
    out = []
    for k in keys:
        x, y, w, h = k["px"]
        o = {"label": k["label"], "hid": k["hid"], "x": q(x), "y": q(y, y_off),
             "w": max(0.25, q(w + gap)), "h": max(0.25, q(h + gap))}
        if len(k["notch"]) == 2 and k["notch"][0] > 0 and k["notch"][1] > 0:
            o["notch"] = [q(k["notch"][0] + gap / 2), q(k["notch"][1] + gap / 2)]
        out.append(o)
    # rounding can leave neighbours overlapping by a hair: trim the left key
    for a in out:
        for b in out:
            if a is not b and b["x"] > a["x"] and b["x"] < a["x"] + a["w"] and                     b["y"] < a["y"] + a["h"] - 0.05 and a["y"] < b["y"] + b["h"] - 0.05 and a["x"] + a["w"] - b["x"] < 0.15:
                a["w"] = round(b["x"] - a["x"], 2)
    for a in out:
        for b in out:
            if a is not b and b["y"] > a["y"] and b["y"] < a["y"] + a["h"] and a["y"] + a["h"] - b["y"] < 0.15 and                     b["x"] < a["x"] + a["w"] - 0.05 and a["x"] < b["x"] + b["w"] - 0.05:
                a["h"] = round(b["y"] - a["y"], 2)
    x0 = min(k["x"] for k in out)
    y0 = min(k["y"] for k in out)
    for k in out:
        k["x"] = round(k["x"] - x0, 2)
        k["y"] = round(k["y"] - y0, 2)
    return out, pitch


# layouts HallJoy uses directly by identity rather than through a named preset
for name, arr, brand in [("IROK NA87 ANSI", "g_irok_na87_ansi", "IROK")]:
    presets.setdefault(name, (arr, brand))

result, missing = {}, []
for name, (arr, brand) in sorted(presets.items()):
    if arr not in arrays:
        missing.append((name, arr))
        continue
    keys, pitch = units(arrays[arr])
    result[name] = {"array": arr, "brand": brand, "pitch": pitch, "keys": keys}
out = os.path.join(os.path.dirname(__file__), "hj_layouts.json")
json.dump(result, open(out, "w", encoding="utf8"), indent=1, ensure_ascii=False)
print(len(arrays), "arrays,", len(presets), "presets,", len(result), "converted")
if missing:
    print("missing arrays:", missing[:10])

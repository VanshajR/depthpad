"""Rust board table for the RongYuan RY5088 stream protocol, from HallJoy's
docs/research/rongyuan-stream/profiles.json (the source of rongyuan_stream_protocol.h)."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import json, re

HJ = HALLJOY
OUT = os.path.join(CORE, "src", "protocols", "halljoy", "rongyuan_tables.rs")
profiles = json.load(open(HJ + r"\docs\research\rongyuan-stream\profiles.json", encoding="utf8"))
header = open(HJ + r"\src\HallJoyProject\HallJoy\rongyuan_stream_protocol.h", encoding="utf8").read()

# the compiled header is what HallJoy ships: check the profiles agree with it
compiled = {}
for m in re.finditer(r'\{(\d+),(\d+),(\d+),L"[^"]*","([^"]+)",(\d+),(true|false),\{\{([\d,]+)\}\}\}', header):
    compiled[m.group(4)] = (int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(5)), m.group(6) == "true",
                            [int(v) for v in m.group(7).split(",")])
assert len(compiled) == len(profiles), (len(compiled), len(profiles))
aliases = [tuple(int(v) for v in a.split(",")) for a in
           re.findall(r"\{(\d+,\d+,\d+,\d+,\d+,\d+)\}", header[header.index("kUsbAliases"):header.index("inline const Model* Find")])]


def decode(m, slot):
    p = m[slot * 4:slot * 4 + 4]
    if p[0] == 0 and p[1] == 0 and p[3] == 0:
        return p[2]
    if p[0] == 10 and p[1] == 1:
        return 0x409
    return 0


def slots(matrix):
    return [(s, decode(matrix, s)) for s in range(128) if decode(matrix, s)]


lines = ["//! Generated from HallJoy's docs/research/rongyuan-stream/profiles.json (AGPL-3.0). Do not edit.",
         "//! Each board: firmware board number, USB IDs, product name, full travel (µm), whether its",
         "//! travel units must come from the keyboard, and its key matrix (slot, analog code).",
         "", "use super::rongyuan::Board;", "", "pub const BOARDS: &[Board] = &["]
for p in profiles:
    c = compiled[p["product"]]
    assert c[:5] == (p["board"], p["vid"], p["pid"], p["range_um"], p["precision_enum"]), p["product"]
    assert c[5] == p["matrix"], p["product"]
    keys = ", ".join(f"({s}, {k})" for s, k in slots(p["matrix"]))
    lines.append(f'    Board {{ board: {p["board"]}, vid: 0x{p["vid"]:04X}, pid: 0x{p["pid"]:04X}, product: "{p["product"]}", '
                 f'range_um: {p["range_um"]}, precision_enum: {str(p["precision_enum"]).lower()}, keys: &[{keys}] }},')
lines += ["];", "", "/// Other USB IDs a board is sold under: (board, vid, pid) -> (vid, pid, board) of its entry.",
          "pub const ALIASES: &[((u32, u16, u16), (u16, u16, u32))] = &["]
lines += [f"    (({a[0]}, 0x{a[1]:04X}, 0x{a[2]:04X}), (0x{a[3]:04X}, 0x{a[4]:04X}, {a[5]}))," for a in aliases]
lines += ["];", ""]
open(OUT, "w", encoding="utf8", newline="\n").write("\n".join(lines))
print(len(profiles), "boards,", len(aliases), "aliases")

# ------------------------------------------------------------------ Attack Shark (polled pages)
shark = open(HJ + r"\src\HallJoyProject\HallJoy\attackshark_pro_native_model.h", encoding="utf8").read()
rows = re.findall(r'\{(\d+),0x([0-9a-f]+),(\d+),L"([^"]+)",\s*\{([\d,]+)\},\s*\{([\d,]+)\}\}', shark)
assert len(rows) == 37, len(rows)
out = ["", "/// Attack Shark boards (read by polling): board, product ID, Fn slot, factory key per slot.",
       "pub const SHARK: &[(u32, u16, u8, &[(u8, u16)])] = &["]
for board, pid, fn_slot, name, factory, _fn in rows:
    f = [int(v) for v in factory.split(",")]
    assert len(f) == 128
    keys = ", ".join(f"({s}, {c})" for s, c in enumerate(f) if c)
    out.append(f"    ({board}, 0x{int(pid, 16):04X}, {fn_slot}, &[{keys}]), // {name}")
out += ["];", ""]
open(OUT, "a", encoding="utf8", newline="\n").write("\n".join(out))
print(len(rows), "Attack Shark boards")

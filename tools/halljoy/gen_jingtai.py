"""Rust table for the JingTai boards (IROK, WLMOUSE, IYX, CAROTMAS) on the 0x5C frame protocol, from
HallJoy's mg75_pro_protocol.h, wlmouse_ying75_protocol.h and jingtai_v1_profiles.h."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import re

HJ = HALLJOY_SRC
OUT = os.path.join(CORE, "src", "protocols", "halljoy", "jingtai_tables.rs")
mg = open(HJ + r"\mg75_pro_protocol.h", encoding="utf8").read()
ying = open(HJ + r"\wlmouse_ying75_protocol.h", encoding="utf8").read()
jt = open(HJ + r"\jingtai_v1_profiles.h", encoding="utf8").read()


def nums(s):
    return [int(v) for v in re.findall(r"\d+", s)]


models = {}  # identity -> (name, range, factory_check, lenient, actions, usb list)
mg_actions = nums(re.search(r"kFactoryActions = \{\s*\{(.*?)\}\};", mg, re.S).group(1))
assert len(mg_actions) == 126
models["MG75PRO-1CA5-0807"] = ("IROK MG75 Pro", 3500, True, False, mg_actions, [(0x1CA5, 0x0807, "IROK MG75 PRO")])
m = re.search(r'kModel\{"([^"]+)", L"([^"]+)", \{\{(.*?)\}\}, (\d+), (\d+)\}', ying, re.S)
y_actions = nums(m.group(3))
assert len(y_actions) == 126
models[m.group(1)] = (m.group(2), int(m.group(5)), True, True, y_actions, [(0x36A7, 0xF887, "WLKB YING 75")])
base = {}
for m in re.finditer(r'Model (model_\w+)\{"([^"]+)",L"([^"]+)",\{\{([\d,]+)\}\},(\d+),(\d+)\}', jt):
    acts = nums(m.group(4))
    assert len(acts) == 126
    base[m.group(1)] = (m.group(3), acts)
    models[m.group(2)] = (m.group(3), int(m.group(6)), False, False, acts, [])
for m in re.finditer(r'Model (model_\w+)\{"([^"]+)",(\w+)\.name,\w+\.actions,\w+\.count,(\d+)\}', jt):
    name, acts = base[m.group(3)]
    models[m.group(2)] = (name, int(m.group(4)), False, False, acts, [])
    base[m.group(1)] = (name, acts)
ident_of = {}
for var, ident in re.findall(r"Model (model_\w+)\{\"([^\"]+)\"", jt):
    ident_of[var] = ident
for vid, pid, product, var in re.findall(r'\{(\d+),(\d+),L"([^"]+)",&(model_\w+)\}', jt):
    models[ident_of[var]][5].append((int(vid), int(pid), product))
lines = ["//! Generated from HallJoy's mg75_pro_protocol.h, wlmouse_ying75_protocol.h and jingtai_v1_profiles.h",
         "//! (AGPL-3.0). Do not edit.", "", "use super::sparkplay::JingTai;", "", "pub const MODELS: &[JingTai] = &["]
for ident, (name, rng, check, lenient, acts, usb) in models.items():
    assert usb, ident
    u = ", ".join(f'(0x{v:04X}, 0x{p:04X}, "{n}")' for v, p, n in usb)
    lines.append(f"    // {name}")
    lines.append(f'    JingTai {{ identity: "{ident}", range_um: {rng}, factory_check: {str(check).lower()}, '
                 f'lenient: {str(lenient).lower()}, usb: &[{u}], actions: &[{", ".join(map(str, acts))}] }},')
lines += ["];", ""]
open(OUT, "w", encoding="utf8", newline="\n").write("\n".join(lines))
print(len(models), "models:", list(models))

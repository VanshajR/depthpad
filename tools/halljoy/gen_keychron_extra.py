"""Keychron K4/K6/Q2/Q4 HE and Lemokey P1 HE ISO key matrices from HallJoy's copy of the AnalogSense
plugin (third_party/UniversalAnalogPluginFixed), appended to analogsense_tables.rs."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import re

JS = ANALOGSENSE_JS
UAP = os.path.join(HALLJOY, "third_party", "UniversalAnalogPluginFixed")
OUT = os.path.join(CORE, "src", "protocols", "analogsense_tables.rs")
consts = {k: int(v, 16) for k, v in re.findall(r"const (KEY_[A-Z0-9_]+) = (0x[0-9a-fA-F]+);", open(JS, encoding="utf8").read())}
consts["KEY_NONE"] = 0
assert consts["KEY_OEM_1"] == 0x403 and consts["KEY_OEM_2"] == 0x404
for n in range(1, 9):
    consts.setdefault(f"KEY_OEM_{n}", 0x402 + n)
cpp = open(UAP + r"\overlay\Soup\soup\AnalogueKeyboard.cpp", encoding="utf8").read()
maps = open(UAP + r"\halljoy_keychron_known_maps.h", encoding="utf8").read()


def table(src, name):
    body = re.search(r"\b" + name + r"\[\] = \{ (\d+), (\d+),(.*?)\};", src, re.S)
    rows, cols = int(body.group(1)), int(body.group(2))
    vals = []
    for tok in re.findall(r"hid_scancode_to_soup_key\((\d+)\)|(KEY_[A-Z0-9_]+)", re.sub(r"/\*.*?\*/", "", body.group(3))):
        vals.append(int(tok[0]) if tok[0] else consts[tok[1]])
    assert len(vals) == rows * cols, (name, len(vals), rows * cols)
    return rows, cols, vals


out = ["", "// From HallJoy's copy of the AnalogSense plugin (third_party/UniversalAnalogPluginFixed, MIT / AGPL-3.0):",
       "// Keychron K4/K6/Q2/Q4 HE and Lemokey P1 HE ISO."]
for rust, src, name in [("KEYCHRON_K4_HE", cpp, "layout_keychron_k4_he_ansi"), ("LEMOKEY_P1_HE_ISO", cpp, "layout_lemokey_p1_he_iso"),
                        ("KEYCHRON_K6_HE", maps, "halljoy_k6_ansi"), ("KEYCHRON_Q2_HE", maps, "halljoy_q2_ansi"),
                        ("KEYCHRON_Q4_HE", maps, "halljoy_q4_ansi")]:
    r, c, v = table(src, name)
    out.append(f"pub const {rust}: (usize, usize, &[u16]) = ({r}, {c}, &[{', '.join(f'0x{x:X}' for x in v)}]);")
s = open(OUT, encoding="utf8").read()
s = s.split("\n// From HallJoy's copy")[0].rstrip("\n") + "\n"
open(OUT, "w", encoding="utf8", newline="\n").write(s + "\n".join(out) + "\n")
print("ok")

"""Rust model table for the Hex80 (ATK / MADLIONS, QMK raw HID) protocol, from HallJoy's
hex80_protocol.h and generated/atk_hex80_family.h."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import re

HJ = HALLJOY_SRC
OUT = os.path.join(CORE, "src", "protocols", "halljoy", "hex80_tables.rs")
text = open(HJ + r"\hex80_protocol.h", encoding="utf8").read() + open(HJ + r"\generated\atk_hex80_family.h", encoding="utf8").read()


def array(name):
    m = re.search(r"std::array<std::uint16_t, *\w+> " + name + r"\{\{(.*?)\}\};", text, re.S)
    body = re.sub(r"//[^\n]*", "", m.group(1))
    return [int(v, 0) for v in re.findall(r"0x[0-9a-fA-F]+|\d+", body)]


models = []
for m in re.finditer(r'inline constexpr Model (\w+)\{\s*L"([^"]+)",\s*"([^"]+)",\s*(\w+)\.data\(\),\s*\w+\.size\(\),\s*(\w+),\s*(\w+)\.data\(\),\s*(\d+),\s*(\w+),\s*(true|false)\s*\};', text):
    _, name, product, pids, slots, slot_map, fixed, deadzone, finish = m.groups()
    pid_list = array(pids)
    slot_list = array(slot_map)
    slots = len(slot_list) if not slots.isdigit() else int(slots)
    if slots < len(slot_list):
        slot_list = slot_list[:slots]
    deadzone = 8 if deadzone == "kRawDeadzone" else int(deadzone)
    models.append((name, product, pid_list, slots, slot_list, int(fixed), deadzone, finish == "true"))
assert len(models) == 17, len(models)

lines = ["//! Generated from HallJoy's hex80_protocol.h and generated/atk_hex80_family.h (AGPL-3.0). Do not edit.",
         "", "use super::hex80::Model;", "", "pub const MODELS: &[Model] = &["]
for name, product, pids, slots, slot_list, fixed, deadzone, finish in models:
    keys = ", ".join(str(v) for v in slot_list)
    lines.append(f'    // {name}')
    lines.append(f'    Model {{ product: "{product}", pids: &[{", ".join(f"0x{p:04X}" for p in pids)}], '
                 f'fixed_max: {fixed}, deadzone: {deadzone}, calibration_finish: {str(finish).lower()}, slots: &[{keys}] }},')
lines += ["];", ""]
open(OUT, "w", encoding="utf8", newline="\n").write("\n".join(lines))
print(len(models), "models:", [(m[1], len(m[2])) for m in models])

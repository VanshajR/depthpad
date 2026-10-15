"""Rust tables for the IPI / BY addressed protocol, from HallJoy's generated/ipi_models.h."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import re

SRC = os.path.join(HALLJOY_SRC, "generated", "ipi_models.h")
OUT = os.path.join(CORE, "src", "protocols", "halljoy", "ipi_tables.rs")
t = open(SRC, encoding="utf8").read()
hids = [int(v) for v in re.search(r"factoryHids\[\] = \{([^}]*)\}", t).group(1).split(",")]
ids = {m.group(1): [int(v) for v in m.group(2).split(",")] for m in re.finditer(r"ids_(\w+)\[\] = \{([^}]*)\}", t)}
models = re.findall(r'\{0x([0-9A-F]+)ull, L"([^"]+)", "([^"]+)", ids_(\w+), (\d+)\}', t)
assert len(models) == 10, len(models)
lines = ["//! Generated from HallJoy's generated/ipi_models.h (AGPL-3.0). Do not edit.", "",
         "/// Factory HID usage of each key ID (0x409 is Fn).",
         f"pub const FACTORY: &[u16] = &[{', '.join(map(str, hids))}];", "",
         "/// (model UUID, product name, key IDs the firmware scans)",
         "pub const MODELS: &[(u64, &str, &[u8])] = &["]
for uuid, name, product, arr, count in models:
    assert len(ids[arr]) == int(count), (arr, len(ids[arr]), count)
    lines.append(f'    (0x{uuid}, "{product}", &[{", ".join(map(str, ids[arr]))}]), // {name}')
lines += ["];", ""]
open(OUT, "w", encoding="utf8", newline="\n").write("\n".join(lines))
print(len(hids), "key IDs,", len(models), "models")

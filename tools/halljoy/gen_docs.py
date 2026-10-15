"""docs/KEYBOARDS.md (every supported board, by brand) and the README's keyboard summary, from the
model and variant files."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401
import glob, json, os, re

ROOT = REPO
DATA = os.path.join(ROOT, "crates", "depthpad-core", "data")
SOURCE = {  # driver -> where the protocol comes from
    "sonix-ajazz": "Depthpad / HallJoy", "replay": None,
    "wooting": "AnalogSense", "razer": "AnalogSense", "nuphy": "AnalogSense", "drunkdeer": "AnalogSense",
    "keychron": "AnalogSense", "madlions": "AnalogSense", "bytech": "AnalogSense",
}
devices = {}
for f in glob.glob(os.path.join(DATA, "devices", "*.json")):
    d = json.load(open(f, encoding="utf8"))
    devices[d["id"]] = d
boards = []  # (vendor, name, status, source, note)
for d in devices.values():
    src = SOURCE.get(d["driver"], "HallJoy")
    vf = os.path.join(DATA, "variants", d["id"] + ".json")
    if os.path.exists(vf):
        for v in json.load(open(vf, encoding="utf8"))["variants"]:
            boards.append((v.get("vendor", d["vendor"]), v["name"], d["status"], src, d.get("note")))
    elif d["driver"] != "replay":
        boards.append((d["vendor"], d["name"], d["status"], src, d.get("note")))


def canon(v):
    return {"AULA": "AULA", "Aula": "AULA", "MADLIONS": "MadLions", "MadLions": "MadLions", "ATTACK SHARK": "Attack Shark",
            "XINMENG": "Xinmeng"}.get(v, v)


by = {}
for vendor, name, status, src, note in boards:
    entries = by.setdefault(canon(vendor), [])
    if not any(e[0] == name for e in entries):  # variants told apart only inside the app
        entries.append((name, status, src, note))
boards = [b for v in by.values() for b in v]


def short(vendor, name):
    """A board's name without its brand in front."""
    return name[len(vendor) + 1:] if name.lower().startswith(vendor.lower() + " ") else name

lines = ["# Supported keyboards", "",
         "Generated from the model files in `crates/depthpad-core/data/`. **Tested** means a maintainer has used",
         "it with Depthpad; **untested** means the support is written from working code in",
         "[HallJoy](https://github.com/PashOK7/HallJoy) or [AnalogSense](https://github.com/AnalogSense/JavaScript-SDK)",
         "but nobody has tried it with Depthpad yet. If you own one, please",
         "[tell us how it went](https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml).", "",
         f"{len(boards)} boards from {len(by)} brands.", ""]
for vendor in sorted(by, key=str.lower):
    lines += [f"## {vendor}", ""]
    for name, status, src, note in sorted(by[vendor], key=lambda b: b[0].lower()):
        tag = "**tested**" if status == "tested" else status
        extra = f" ({note})" if note and src == "AnalogSense" and ("Synapse" in note or "firmware" in note) else ""
        lines.append(f"- {name}: {tag}, from {src}{extra}")
    lines.append("")
open(os.path.join(ROOT, "docs", "KEYBOARDS.md"), "w", encoding="utf8", newline="\n").write("\n".join(lines))

# README summary between markers
summary = ["<!-- keyboards:start (generated) -->",
           f"Depthpad supports **{len(boards)} keyboards from {len(by)} brands**; the full list, model by model, is in",
           "[docs/KEYBOARDS.md](docs/KEYBOARDS.md).", "",
           "| Brand | Boards |", "|---|---|"]
big = [v for v in by if len(by[v]) >= 4]
for vendor in sorted(big, key=lambda v: (-len(by[v]), v.lower())):
    names = sorted({short(vendor, n) for n, *_ in by[vendor]}, key=str.lower)
    shown = ", ".join(names[:5]) + (f" and {len(names) - 5} more" if len(names) > 5 else "")
    if any(s == "tested" for _, s, *_ in by[vendor]):
        shown = "**AK680 V2 HE (tested)**, " + shown.replace("AK680 V2 HE, ", "")
    summary.append(f"| {vendor} | {shown} |")
rest = sorted((v for v in by if v not in big), key=str.lower)
summary += ["", "Also: " + ", ".join(rest) + "."]
summary.append("<!-- keyboards:end -->")
p = os.path.join(ROOT, "README.md")
s = open(p, encoding="utf8").read()
if "<!-- keyboards:start" in s:
    s = re.sub(r"<!-- keyboards:start.*?<!-- keyboards:end -->", "\n".join(summary), s, flags=re.S)
else:
    a = s.index("| Keyboard | Status |")
    b = s.index("\n\n", s.index("| Redragon K709 HE | Untested |"))
    s = s[:a] + "\n".join(summary) + s[b:]
open(p, "w", encoding="utf8", newline="\n").write(s)
print(len(boards), "boards,", len(by), "brands")

"""The keyboard families ported from HallJoy: driver, USB match, a typical layout, and the boards
(variants) with the firmware names HallJoy's identity catalogue (generated/layout_pipeline/
identities.h) and protocol code give for them."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ANALOGSENSE_JS, CORE, HALLJOY, HALLJOY_SRC, REPO  # noqa: E402,F401


def usb(vid=None, pid=None, page=None, usage=None):
    m = {}
    if vid:
        m["vid"] = vid
    if pid:
        m["pid"] = pid
    if page:
        m["usage_page"] = page
    if usage:
        m["usage"] = usage
    return m


FAMILIES = [
    {
        "id": "aula-win-he", "name": "AULA WIN / Redragon / IROK HE keyboard", "vendor": "AULA", "driver": "aula-w669",
        # HallJoy finds these by their vendor collection alone; the brands use different USB IDs
        "usb": [usb(page="FF1B", usage="0091")],
        "layout": "Aula WIN 68 HE Standard ANSI",
        "variants": [
            {"name": "AULA WIN 60 HE", "layout": "Aula WIN 60 HE Standard ANSI",
             "identity": ["SI2825HEARGB", "SI2825KRT12HEARGB", "SI2825KR-AHEARGB", "SI2825KZHEARGB"]},
            {"name": "AULA WIN 68 HE", "layout": "Aula WIN 68 HE Standard ANSI",
             "identity": ["SI2828HEARGB", "SI2828KZHEARGB"]},
            {"name": "AULA KP-TE153", "layout": "Aula KP-TE153 ISO", "identity": ["SI2851UKKZHEARGB"]},
            {"name": "Redragon K673 HE (US)", "vendor": "Redragon", "layout": "Redragon K673WB-RGB-M ANSI",
             "identity": ["7272USHEXYXK673JCARGB"]},
            {"name": "Redragon K673 HE (UK)", "vendor": "Redragon", "layout": "Redragon K673RGB-M ISO",
             "identity": ["7272UKHEXYXBJCARGB"]},
            {"name": "Redragon K673 HE (BR)", "vendor": "Redragon", "layout": "Redragon K673RGB-M ABNT2",
             "identity": ["7272BRHEXYXK673JCARGB"]},
            {"name": "Redragon K686 HE (US)", "vendor": "Redragon", "layout": "Redragon K686BG-RGB-M ANSI",
             "identity": ["7431USHEXYXK686JCARGB"]},
            {"name": "Redragon K686 HE (BR)", "vendor": "Redragon", "layout": "Redragon K686BG-RGB-M ISO",
             "identity": ["7431BRHEXYXJCARGB"]},
            {"name": "Redragon K686 HE (UK)", "vendor": "Redragon", "layout": "Redragon K686RGB-M ISO",
             "identity": ["7431UKHEXYXBJCARGB"]},
            {"name": "Redragon K617 HE", "vendor": "Redragon", "layout": "Redragon K617 HE ANSI",
             "identity": ["7153USHEXYXCPARGB", "7153BRHEXYXCPARGB"]},
            {"name": "IROK NA87", "vendor": "IROK", "layout": "IROK NA87 ANSI", "identity": ["GK8260HERGB"],
             "travel_mm": 4.0},
            {"id": "ajazz-ak820-max-he-rgb", "name": "AJAZZ AK820 MAX HE (wired RGB, 0416:7372)", "vendor": "AJAZZ",
             "layout": "AJAZZ AK820 MAX HE ANSI", "identity": ["SG8994HERGB"],
             "note": "The Nuvoton revision HallJoy supports, firmware V1.13.17. Key range is learned as you press."},
        ],
    },
]

# ------------------------------------------------------------------------------------- RongYuan
import json as _json, re as _re

_HJ = HALLJOY
_IDS = [(_m.group(1), _m.group(2), _m.group(3)) for _m in _re.finditer(
    r'\{"([a-z0-9-]+)", "([^"]+)", 0x[0-9A-F]+ull, L"([^"]+)"\}',
    open(_HJ + r"\src\HallJoyProject\HallJoy\generated\layout_pipeline\identities.inc", encoding="utf8").read())]


def presets(protocol):
    """Firmware name -> HallJoy layout preset, for one protocol."""
    return {product: preset for proto, product, preset in _IDS if proto == protocol}


def _rongyuan():
    from gen import matrix_layout  # noqa: E402 (gen imports this module first)
    profiles = _json.load(open(_HJ + r"\docs\research\rongyuan-stream\profiles.json", encoding="utf8"))
    header = open(_HJ + r"\src\HallJoyProject\HallJoy\rongyuan_stream_protocol.h", encoding="utf8").read()
    aliases = [tuple(int(v) for v in a.split(",")) for a in _re.findall(r"\{(\d+,\d+,\d+,\d+,\d+,\d+)\}",
               header[header.index("kUsbAliases"):header.index("inline const Model* Find")])]
    named = presets("rongyuan-stream")
    groups = {}
    for p in profiles:
        m = p["matrix"]
        codes = {}
        for slot in range(128):
            q = m[slot * 4:slot * 4 + 4]
            c = q[2] if q[0] == 0 and q[1] == 0 and q[3] == 0 else 0x409 if q[0] == 10 and q[1] == 1 else 0
            if c:
                codes[(slot % 6, slot // 6)] = c
        layout = named.get(p["product"])
        key = (p["brand"], p["model"], layout or tuple(sorted(codes.items())), p["range_um"])
        g = groups.setdefault(key, {"name": f'{p["brand"]} {p["model"]}', "vendor": p["brand"],
                                    "layout": layout or matrix_layout(codes), "identity": [],
                                    "travel_mm": p["range_um"] / 1000})
        g["identity"].append(p["product"])
    # Attack Shark boards: polled, fixed 3.5 mm travel, matched by board number
    ident = open(_HJ + r"\src\HallJoyProject\HallJoy\attackshark_pro_layout_identity.h", encoding="utf8").read()
    consts = dict(_re.findall(r'inline constexpr wchar_t (\w+)\[\] = L"([^"]+)";', ident))
    for m in _re.finditer(r'\{(\d+),0x[0-9A-F]+ull,(L"([^"]+)"|(\w+))\}', ident):
        preset = m.group(3) or consts[m.group(4)]
        g = groups.setdefault(("shark", preset), {"name": preset.replace(" ANSI", "").replace("ATTACK SHARK", "Attack Shark"),
                                                  "vendor": "Attack Shark", "layout": preset, "identity": [],
                                                  "travel_mm": 3.5})
        g["identity"].append(f"SHARK-{m.group(1)}")
    variants = list(groups.values())
    # the same name twice (different matrices or ranges): tell them apart by board number
    for v in variants:
        if sum(1 for o in variants if o["name"] == v["name"]) > 1:
            v["id"] = _re.sub(r"[^a-z0-9]+", "-", f'{v["name"]} {v["identity"][0]}'.lower()).strip("-")
    ids = sorted({(p["vid"], p["pid"]) for p in profiles} | {(a[1], a[2]) for a in aliases})
    matches = [usb(f"{v:04X}", f"{pid:04X}", page, "0002") for v, pid in ids for page in ("FFFF", "FF00")]
    return {
        "id": "rongyuan-he", "name": "RongYuan-based HE keyboard", "vendor": "RongYuan", "driver": "rongyuan",
        "usb": matches, "travel_mm": 4.0, "layout": named["RY1B-2352"],
        "note": "Sold under many brands (EPOMAKER, MonsGeek, Akko, Attack Shark, EWEADN, AJAZZ and more). Close the maker's "
                "driver app first: it takes the keyboard's control channel.",
        "variants": variants,
    }


FAMILIES.append(_rongyuan())

# ------------------------------------------------------------------- SparkPlayJoy (AULA, GravaStar)
FAMILIES.append({
    "id": "sparkplay-he", "name": "AULA / GravaStar HE keyboard (SparkPlayJoy)", "vendor": "AULA", "driver": "sparkplay",
    "usb": [usb(v, p, "FFA0", "0001") for v, p in
            [("1CA2", "1902"), ("1CA2", "1901"), ("1CA5", "0409"), ("1CA5", "2201"), ("1CA5", "2202"), ("1CA2", "2201")]],
    "travel_mm": 3.4, "layout": "Aula WIN 60 HE MAX ANSI",
    "variants": [
        {"name": "AULA WIN 60 HE MAX / PRO", "layout": "Aula WIN 60 HE MAX ANSI", "identity": ["0A021902"]},
        {"name": "AULA WIN 68 HE PRO / MAX", "layout": "Aula WIN 68 HE PRO / MAX ANSI", "identity": ["06021901"]},
        {"name": "AULA HERO 68 HE PRO", "layout": "Aula HERO 68 HE PRO ANSI", "identity": ["06050409"]},
        {"name": "GravaStar Mercury V75", "vendor": "GravaStar", "layout": "GravaStar Mercury V75 ANSI", "identity": ["16052201"]},
        {"name": "GravaStar Mercury V75 Pro", "vendor": "GravaStar", "layout": "GravaStar Mercury V75 Pro ANSI", "identity": ["16052202"]},
        {"name": "GravaStar Mercury V75 Lite", "vendor": "GravaStar", "layout": "GravaStar Mercury V75 Lite ANSI", "identity": ["2E022201"]},
    ],
})

# ----------------------------------------------------------------------------- Hex80 (ATK, MADLIONS)
def _hex80():
    src = open(_HJ + r"\src\HallJoyProject\HallJoy\hex80_protocol.h", encoding="utf8").read() + \
        open(_HJ + r"\src\HallJoyProject\HallJoy\generated\atk_hex80_family.h", encoding="utf8").read()
    pids = set()
    for arr in _re.findall(r"std::array<std::uint16_t, *\d+> k\w*(?:Pids|ProductIds)\{\{([^}]*)\}\}", src):
        pids |= {int(v, 16) for v in _re.findall(r"0x([0-9A-Fa-f]+)", arr)}
    groups = {}
    for product, preset in presets("hex80").items():
        g = groups.setdefault(preset, {"name": preset.replace(" ANSI", ""), "layout": preset, "identity": []})
        g["identity"].append(product)
        if preset.startswith("MADLIONS"):
            g["vendor"] = "MADLIONS"
            g["name"] = "MADLIONS MAD68 HE V2"
    return {
        "id": "atk-he", "name": "ATK / MADLIONS HE keyboard (Hex80 protocol)", "vendor": "ATK", "driver": "hex80",
        "usb": [usb("373B", f"{p:04X}", "FF60", "0061") for p in sorted(pids)],
        "travel_mm": 3.3, "layout": "ATK Hex80 ANSI", "variants": list(groups.values()),
    }


FAMILIES.append(_hex80())

# ------------------------------------------------------------------------- IPI / Royal Kludge (BY)
def _ipi():
    groups = {}
    for product, preset in presets("ipi-addressed").items():
        vendor = "Royal Kludge" if preset.startswith("Royal Kludge") else "IPI"
        g = groups.setdefault(preset, {"name": preset.replace(" ANSI", "").replace(" ISO", " (ISO)"), "vendor": vendor,
                                       "layout": preset, "identity": []})
        g["identity"].append(product)
    return {
        "id": "ipi-he", "name": "IPI / Royal Kludge HE keyboard", "vendor": "IPI", "driver": "ipi",
        # BY firmware: the model is told by the UUID it reports, not the USB ID
        "usb": [usb("372E", None, "FF60", "0061")],
        "travel_mm": 3.5, "layout": "IPI QBZ75 + Aurora 75 ANSI", "variants": list(groups.values()),
    }


FAMILIES.append(_ipi())


def _add_jingtai():
    """IROK / WLMOUSE / IYX / CAROTMAS (JingTai firmware): the same driver as SparkPlayJoy."""
    fam = next(f for f in FAMILIES if f["id"] == "sparkplay-he")
    tables = open(os.path.join(CORE, "src", "protocols", "halljoy", "jingtai_tables.rs"),
                  encoding="utf8").read()
    ranges, usbs = {}, set()
    for m in _re.finditer(r'identity: "([^"]+)", range_um: (\d+).*?usb: &\[([^\]]*)\]', tables):
        ranges[m.group(1)] = int(m.group(2))
        usbs |= set(_re.findall(r"\(0x([0-9A-F]{4}), 0x([0-9A-F]{4}),", m.group(3)))
    fam["usb"] += [usb(v, p, "FFA0", "0001") for v, p in sorted(usbs)]
    groups = {}
    for product, preset in presets("irok-mg75-pro").items():
        travel = ranges[product] / 1000
        vendor = preset.split()[0]
        name = preset.replace(" ANSI", "") + (" (cyan switches)" if product.endswith("CYAN") else "")
        g = groups.setdefault((preset, travel), {"name": name, "vendor": vendor, "layout": preset, "identity": [],
                                                 "travel_mm": travel})
        g["identity"].append(product)
    fam["variants"] += list(groups.values())
    fam["name"] = "AULA / GravaStar / IROK HE keyboard"


_add_jingtai()

# ------------------------------------------------------------------------ SparkLink (JingTai V2)
FAMILIES.append({"id": "irok-mg75-max", "name": "IROK MG75 Max", "vendor": "IROK", "driver": "sparklink",
                 "usb": [usb("1CA6", "0529", "FFB0", "0001")], "travel_mm": 3.5, "layout": "IROK MG75 Max ANSI"})
FAMILIES.append({"id": "everglide-su75-pro", "name": "Everglide SU75 Pro", "vendor": "Everglide", "driver": "sparklink",
                 "usb": [usb("1CA6", "3002", "FFB0", "0001")], "travel_mm": 3.5, "layout": "Everglide SU75 Pro ANSI"})

# ------------------------------------------------------------------------------- SteelSeries Apex
FAMILIES.append({"id": "steelseries-apex-pro", "name": "SteelSeries Apex Pro", "vendor": "SteelSeries",
                 "driver": "steelseries-apex", "usb": [usb("1038", "1610", "FFC0", "0001")], "travel_mm": 4.0,
                 "layout": "SteelSeries Apex Pro ANSI",
                 "note": "Needs firmware 4.16.8. Close SteelSeries GG first: it holds the keyboard's analog channel."})

# ----------------------------------------------------------------------------------- SayoDevice
FAMILIES.append({"id": "sayodevice-o3c", "name": "SayoDevice O3C", "vendor": "SayoDevice", "driver": "sayo",
                 "usb": [usb("8089", "0009", "FF12", "0002")], "travel_mm": 4.0, "layout": "SayoDevice O3C ANSI"})

# --------------------------------------------- Keychron / Lemokey models from HallJoy's AnalogSense copy
for _id, _name, _vid, _pid, _layout in [
    ("keychron-k4-he", "Keychron K4 HE", "3434", "0E40", "Keychron K4 HE ANSI - Imported"),
    ("keychron-k6-he", "Keychron K6 HE", "3434", "0E60", "Keychron K6 HE ANSI"),
    ("keychron-q2-he", "Keychron Q2 HE", "3434", "0B20", "Keychron Q2 HE ANSI"),
    ("keychron-q4-he", "Keychron Q4 HE", "3434", "0B40", "Keychron Q4 HE ANSI"),
    ("lemokey-p1-he-iso", "Lemokey P1 HE (ISO)", "362D", "0611", "Lemokey P1 HE ISO"),
]:
    FAMILIES.append({"id": _id, "name": _name, "vendor": _name.split()[0], "driver": "keychron",
                     "usb": [usb(_vid, _pid, "FF60", "0061")], "travel_mm": 4.0, "layout": _layout,
                     "note": "Official firmware is polled key by key, which can lag; the custom firmware at analogsense.org/firmware sends full analog reports."})

# -------------------------------------------------------------------------------------- DrunkDeer
FAMILIES.append({
    "id": "drunkdeer", "name": "DrunkDeer keyboard", "vendor": "DrunkDeer", "driver": "drunkdeer",
    "usb": [usb("352D", None, "FF00")], "travel_mm": 3.6, "layout": "DrunkDeer A75 ANSI",
    "note": "Any DrunkDeer board; the exact model is read from the keyboard.",
    "variants": [{"name": n.replace(" ANSI", ""), "layout": n, "identity": [n]} for n in [
        "DrunkDeer A75 ANSI", "DrunkDeer A75 Pro", "DrunkDeer A75 ISO", "DrunkDeer G60 ANSI", "DrunkDeer G65 ANSI",
        "DrunkDeer G75 ANSI", "DrunkDeer G75 JIS"]],
})

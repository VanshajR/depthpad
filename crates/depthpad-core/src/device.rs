//! Supported keyboard models. Each model is a data file in `data/devices/` that says how to find
//! the keyboard (USB IDs and the HID collection its driver talks to), which driver runs it, and
//! its keys: their positions (the driver's own key numbers), the HID usage each sends from the
//! factory, the `KeyboardEvent.code`-style name profiles use, and how to draw them.
//!
//! Adding a keyboard whose protocol is already implemented is just a new data file (see
//! CONTRIBUTING.md); a new protocol also needs a driver in `platform/drivers/`.
//!
//! Keyboard families: many boards share one protocol and one USB ID and only differ in what their
//! firmware calls itself. Such a family is one model in `data/devices/` (found by USB ID, drawn
//! with a typical layout), and its boards are variants in `data/variants/<family>.json`, each with
//! the firmware names (`identity`) it answers to. The driver reads the name after connecting and
//! the app switches to that variant. Variants are only parsed when one is needed.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Deserializer, Serialize};

use crate::keymap::{KeyInfo, Keymap};

/// The bundled model definitions. Keep in sync with `data/devices/`.
const BUNDLED: &[&str] = &[
    include_str!("../data/devices/ajazz-ak680-v2-he.json"),
    include_str!("../data/devices/aula-mini60-he.json"),
    include_str!("../data/devices/aula-mini60-he-pro.json"),
    include_str!("../data/devices/aula-mini60-he-max.json"),
    include_str!("../data/devices/ajazz-ak820-max-he.json"),
    include_str!("../data/devices/wooting.json"),
    include_str!("../data/devices/razer-huntsman-v2-analog.json"),
    include_str!("../data/devices/razer-huntsman-mini-analog.json"),
    include_str!("../data/devices/razer-huntsman-v3-pro.json"),
    include_str!("../data/devices/razer-huntsman-v3-pro-tkl.json"),
    include_str!("../data/devices/razer-huntsman-v3-pro-mini.json"),
    include_str!("../data/devices/nuphy-he.json"),
    include_str!("../data/devices/drunkdeer.json"),
    include_str!("../data/devices/keychron-q1-he.json"),
    include_str!("../data/devices/keychron-q3-he.json"),
    include_str!("../data/devices/keychron-q5-he.json"),
    include_str!("../data/devices/keychron-k2-he.json"),
    include_str!("../data/devices/lemokey-p1-he.json"),
    include_str!("../data/devices/madlions-mad60he.json"),
    include_str!("../data/devices/madlions-mad68he.json"),
    include_str!("../data/devices/redragon-k709-he.json"),
    include_str!("../data/devices/aula-win-he.json"),
    include_str!("../data/devices/rongyuan-he.json"),
    include_str!("../data/devices/sparkplay-he.json"),
    include_str!("../data/devices/atk-he.json"),
    include_str!("../data/devices/ipi-he.json"),
    include_str!("../data/devices/irok-mg75-max.json"),
    include_str!("../data/devices/everglide-su75-pro.json"),
    include_str!("../data/devices/steelseries-apex-pro.json"),
    include_str!("../data/devices/sayodevice-o3c.json"),
    include_str!("../data/devices/keychron-k4-he.json"),
    include_str!("../data/devices/keychron-k6-he.json"),
    include_str!("../data/devices/keychron-q2-he.json"),
    include_str!("../data/devices/keychron-q4-he.json"),
    include_str!("../data/devices/lemokey-p1-he-iso.json"),
];

fn hex<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let s = String::deserialize(d)?;
    u16::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

fn hex_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u16>, D::Error> {
    hex(d).map(Some)
}

/// One HID collection that identifies the model. A missing vendor ID, product ID, usage page or
/// usage matches any (e.g. every Wooting keyboard has the same analog usage page; some protocol
/// families are sold under many brands and are found by their vendor collection alone).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsbMatch {
    #[serde(default, deserialize_with = "hex_opt")]
    pub vid: Option<u16>,
    #[serde(default, deserialize_with = "hex_opt")]
    pub pid: Option<u16>,
    #[serde(default, deserialize_with = "hex_opt")]
    pub usage_page: Option<u16>,
    #[serde(default, deserialize_with = "hex_opt")]
    pub usage: Option<u16>,
    /// The collection must have this input report (Razer's analog report 7 or 11).
    #[serde(default)]
    pub report_id: Option<u8>,
}

impl UsbMatch {
    pub fn matches(&self, vid: u16, pid: u16, usage_page: u16, usage: u16) -> bool {
        self.vid.is_none_or(|v| v == vid)
            && self.pid.is_none_or(|p| p == pid)
            && self.usage_page.is_none_or(|p| p == usage_page)
            && self.usage.is_none_or(|u| u == usage)
    }
}

/// Settings for keyboards on the Sonix driver (AJAZZ, AULA), from what their makers' software
/// does. All optional.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SonixOptions {
    /// Bytes requested with the device-info command (AJAZZ's Driveall reads 48, AULA's software 56).
    #[serde(default)]
    pub info_len: Option<u8>,
    /// The maker code the device-info reply must carry (AULA: 0166).
    #[serde(default, deserialize_with = "hex_opt")]
    pub manufacturer: Option<u16>,
    /// The product code the reply must carry, unless the USB product name contains
    /// `product_name` (another keyboard shares the AULA MINI 60 HE PRO's USB IDs).
    #[serde(default, deserialize_with = "hex_opt")]
    pub product: Option<u16>,
    #[serde(default)]
    pub product_name: Option<String>,
    /// How long a key's analog reading counts as current, ms (keyboards that stream every
    /// pressed key need longer than the AK680's single-key stream).
    #[serde(default)]
    pub fresh_ms: Option<u32>,
}

/// How far support has been verified: `tested` on real hardware by a maintainer, `community`
/// reported working by owners, `untested` ported from another project without a device to try.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Tested,
    Community,
    Untested,
}

fn one() -> f64 {
    1.0
}

/// One key in a layout row: `[position, legend, width]`, or an object for anything else,
/// following keyboard-layout-editor.com's conventions so layouts can be copied from there (or
/// from VIA definitions, which use the same format): `x` is a gap before the key, `y` moves this
/// row and the rows after it down, `w`/`h` are the size in key units. `notch` cuts the bottom-left
/// corner out of a tall key (an ISO Enter): `[width, from_top]` in key units.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RowKey {
    Plain(u8, String, f64),
    Full {
        pos: u8,
        #[serde(default)]
        legend: String,
        #[serde(default = "one")]
        w: f64,
        #[serde(default = "one")]
        h: f64,
        #[serde(default)]
        x: f64,
        #[serde(default)]
        y: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notch: Option<[f64; 2]>,
    },
}

/// Where a key is drawn, in key units from the top left.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KeyRect {
    pub pos: u8,
    pub legend: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The bottom-left corner cut out (see `RowKey`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notch: Option<[f64; 2]>,
}

impl KeyRect {
    /// The key as rectangles (x, y, w, h): two for a notched key.
    pub fn parts(&self) -> Vec<(f64, f64, f64, f64)> {
        match self.notch {
            Some([nw, ny]) => vec![(self.x, self.y, self.w, ny), (self.x + nw, self.y + ny, self.w - nw, self.h - ny)],
            None => vec![(self.x, self.y, self.w, self.h)],
        }
    }
}

/// Turn layout rows into key rectangles.
pub fn place(rows: &[Vec<RowKey>]) -> Vec<KeyRect> {
    let mut out = Vec::new();
    let mut y = 0.0;
    for row in rows {
        let mut x = 0.0;
        for k in row {
            let (pos, legend, w, h, dx, dy, notch) = match k {
                RowKey::Plain(p, l, w) => (*p, l.clone(), *w, 1.0, 0.0, 0.0, None),
                RowKey::Full { pos, legend, w, h, x, y, notch } => (*pos, legend.clone(), *w, *h, *x, *y, *notch),
            };
            y += dy;
            x += dx;
            out.push(KeyRect { pos, legend, x, y, w, h, notch });
            x += w;
        }
        y += 1.0;
    }
    out
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// Which driver talks to it (see `platform::drivers`).
    pub driver: String,
    pub status: Status,
    pub usb: Vec<UsbMatch>,
    /// Full key travel in mm.
    pub travel_mm: f64,
    /// Width of the drawing in key units.
    pub width_units: f64,
    /// Layout rows, top to bottom (see `RowKey`).
    #[serde(skip_serializing)]
    pub rows: Vec<Vec<RowKey>>,
    /// The rows placed: what the window draws.
    #[serde(skip_deserializing)]
    pub layout: Vec<KeyRect>,
    /// Height of the drawing in key units.
    #[serde(skip_deserializing)]
    pub height_units: f64,
    #[serde(deserialize_with = "keys")]
    pub keys: Keymap,
    /// Something owners need to know (shown with the keyboard), e.g. software that must run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Sonix driver settings.
    #[serde(default, skip_serializing)]
    pub sonix: SonixOptions,
    /// For a variant: the family model it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// For a variant: the names its firmware reports (see the module docs).
    #[serde(default, skip_serializing)]
    pub identity: Vec<String>,
}

fn keys<'de, D: Deserializer<'de>>(d: D) -> Result<Keymap, D::Error> {
    let raw: BTreeMap<String, KeyInfo> = BTreeMap::deserialize(d)?;
    raw.into_iter()
        .map(|(k, v)| k.parse().map(|p| (p, v)).map_err(serde::de::Error::custom))
        .collect()
}

impl Model {
    fn parse(json: &str) -> Result<Self, String> {
        let mut m: Model = serde_json::from_str(json).map_err(|e| e.to_string())?;
        m.layout = place(&m.rows);
        m.height_units = m.layout.iter().map(|k| k.y + k.h).fold(0.0, f64::max);
        Ok(m)
    }

    /// The family this model belongs to (its own id when it isn't a variant).
    pub fn family_id(&self) -> &str {
        self.family.as_deref().unwrap_or(&self.id)
    }

    /// Analog code (see `KeyInfo::code`) -> position.
    pub fn code_positions(&self) -> HashMap<u16, u8> {
        self.keys.iter().filter(|(_, k)| k.analog_code() != 0).map(|(&p, k)| (k.analog_code(), p)).collect()
    }

    /// Key name (e.g. "KeyW") -> this model's position for it.
    pub fn position_of(&self, name: &str) -> Option<u8> {
        self.keys.iter().find(|(_, k)| k.key == name).map(|(p, _)| *p)
    }

    /// The first USB match this collection satisfies (report IDs are checked by the caller).
    pub fn usb_match(&self, vid: u16, pid: u16, usage_page: u16, usage: u16) -> Option<&UsbMatch> {
        self.usb.iter().find(|m| m.matches(vid, pid, usage_page, usage))
    }

    pub fn matches(&self, vid: u16, pid: u16, usage_page: u16, usage: u16) -> bool {
        self.usb_match(vid, pid, usage_page, usage).is_some()
    }

    /// Whether a device path (as Windows reports it) belongs to this model.
    /// Matches without a vendor ID use the USB IDs of the keyboard the driver actually opened
    /// (see `note_opened`).
    pub fn owns_path(&self, path: &str) -> bool {
        let p = path.to_ascii_lowercase();
        let has = |vid: u16, pid: Option<u16>| match pid {
            Some(pid) => p.contains(&format!("vid_{vid:04x}&pid_{pid:04x}")),
            None => p.contains(&format!("vid_{vid:04x}&")),
        };
        self.usb.iter().any(|m| match m.vid {
            Some(vid) => has(vid, m.pid),
            None => opened().lock().unwrap().iter().any(|(f, v, pid)| f == self.family_id() && has(*v, Some(*pid))),
        })
    }
}

fn opened() -> &'static Mutex<Vec<(String, u16, u16)>> {
    static OPENED: OnceLock<Mutex<Vec<(String, u16, u16)>>> = OnceLock::new();
    OPENED.get_or_init(|| Mutex::new(Vec::new()))
}

/// A driver opened this model's keyboard with these USB IDs (for models matched without a vendor
/// ID: `owns_path` then knows which keyboard's key events are its own).
pub fn note_opened(model: &Model, vid: u16, pid: u16) {
    let mut o = opened().lock().unwrap();
    let entry = (model.family_id().to_string(), vid, pid);
    if !o.contains(&entry) {
        o.push(entry);
    }
}

/// All bundled models, parsed once.
pub fn models() -> &'static [Model] {
    static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
    MODELS.get_or_init(|| {
        BUNDLED
            .iter()
            .map(|json| Model::parse(json).unwrap_or_else(|e| panic!("bundled device definition: {e}")))
            .collect()
    })
}

/// A pretend keyboard that plays back a recorded session (driver "replay"): for trying the app,
/// and multi-keyboard behaviour, without hardware. Only connected when DEPTHPAD_DEMO_KEYBOARD
/// names a recording; never listed otherwise.
pub fn demo() -> &'static Model {
    static DEMO: OnceLock<Model> = OnceLock::new();
    DEMO.get_or_init(|| {
        let mut m = models()[0].clone();
        m.id = "demo-replay".into();
        m.name = "Demo keyboard (recorded session)".into();
        m.vendor = "Depthpad".into();
        m.driver = "replay".into();
        m.usb.clear();
        m
    })
}

pub fn model(id: &str) -> Option<&'static Model> {
    models()
        .iter()
        .find(|m| m.id == id)
        .or_else(|| (id == demo().id).then(demo))
        .or_else(|| VARIANTS.iter().find(|v| v.0 == id).and_then(|v| load_variant(v.1, |m| m.id == id)))
}

/// Generated from `data/variants/`: (variant id, family id, firmware names) for every variant,
/// and each family's variant file.
mod variants {
    include!("../data/variants/index.rs");
}
use variants::{FILES as VARIANT_FILES, INDEX as VARIANTS};

/// Normalised firmware name: upper case, no spaces.
fn norm(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase()
}

/// The variant of a family whose firmware reports this name.
pub fn variant(family: &Model, identity: &str) -> Option<&'static Model> {
    let want = norm(identity);
    let entry = VARIANTS.iter().find(|v| v.1 == family.family_id() && v.2.iter().any(|n| norm(n) == want))?;
    load_variant(entry.1, |m| m.id == entry.0)
}

/// Parse a family's variant file and keep (leak) the variant `pick` chooses, once per variant.
fn load_variant(family_id: &str, pick: impl Fn(&Model) -> bool) -> Option<&'static Model> {
    static LOADED: OnceLock<Mutex<Vec<&'static Model>>> = OnceLock::new();
    let loaded = LOADED.get_or_init(|| Mutex::new(Vec::new()));
    if let Some(m) = loaded.lock().unwrap().iter().find(|m| pick(m)) {
        return Some(m);
    }
    let family = models().iter().find(|m| m.id == family_id)?;
    let json = VARIANT_FILES.iter().find(|f| f.0 == family_id)?.1;
    let m = parse_variants(family, json).unwrap_or_else(|e| panic!("variants of {family_id}: {e}")).into_iter().find(|m| pick(m))?;
    let m: &'static Model = Box::leak(Box::new(m));
    loaded.lock().unwrap().push(m);
    Some(m)
}

/// A family's variant file: `{"variants": [...]}`. Each variant gives its own id, name, layout
/// and keys (and anything else it changes); everything else comes from the family.
fn parse_variants(family: &Model, json: &str) -> Result<Vec<Model>, String> {
    #[derive(Deserialize)]
    struct File {
        variants: Vec<serde_json::Map<String, serde_json::Value>>,
    }
    let base = BUNDLED
        .iter()
        .filter_map(|j| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(j).ok())
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(family.id.as_str()))
        .ok_or("family definition not found")?;
    let file: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
    file.variants
        .into_iter()
        .map(|v| {
            let mut m = base.clone();
            m.remove("note");
            m.extend(v);
            m.insert("family".into(), family.id.clone().into());
            Model::parse(&serde_json::Value::Object(m).to_string())
        })
        .collect()
}

/// The model used when no keyboard is connected and none is chosen.
pub fn fallback() -> &'static Model {
    &models()[0]
}

/// The longest key travel of any supported model, mm (the limit for curve settings).
pub fn max_travel_mm() -> f64 {
    models().iter().map(|m| m.travel_mm).fold(0.0, f64::max)
}

/// Whether any supported model has a key with this name (profiles are shared across models).
pub fn known_key(name: &str) -> bool {
    models().iter().any(|m| m.position_of(name).is_some())
}

// ------------------------------------------------------------------------------- which keyboard

/// One entry per model, in order of first appearance (two of the same model count once).
pub fn unique<'a>(connected: impl IntoIterator<Item = &'a Model>) -> Vec<&'a Model> {
    let mut out: Vec<&Model> = Vec::new();
    for m in connected {
        if !out.iter().any(|o| o.id == m.id) {
            out.push(m);
        }
    }
    out
}

/// Which model to run at start-up, and whether to ask the user: the default if one is set
/// (waiting for it if it's unplugged); otherwise the only connected model; otherwise, with
/// several connected, the first one and ask; with none, the fallback.
pub fn pick<'a>(connected: &[&'a Model], default: Option<&'a Model>, fallback: &'a Model) -> (&'a Model, bool) {
    if let Some(d) = default {
        return (d, false);
    }
    match connected {
        [] => (fallback, false),
        [only] => (only, false),
        [first, ..] => (first, true),
    }
}

#[derive(Debug, PartialEq)]
pub enum Follow<'a> {
    Stay,
    Switch(&'a Model),
    Ask,
}

/// While running, when keyboards come and go: with no default chosen, a current keyboard that
/// is gone is replaced by the only other connected one, or the user is asked if there are
/// several. A default (or a current keyboard that is still here) is never replaced.
pub fn follow<'a>(current: &Model, connected: &[&'a Model], has_default: bool) -> Follow<'a> {
    if has_default || connected.iter().any(|m| m.family_id() == current.family_id()) {
        return Follow::Stay;
    }
    match connected {
        [] => Follow::Stay,
        [only] => Follow::Switch(only),
        _ => Follow::Ask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_consistent() {
        let mut ids = std::collections::HashSet::new();
        let mut all: Vec<Model> = models().to_vec();
        for (family, json) in VARIANT_FILES {
            let f = models().iter().find(|m| m.id == *family).unwrap_or_else(|| panic!("no family {family}"));
            let vs = parse_variants(f, json).unwrap_or_else(|e| panic!("variants of {family}: {e}"));
            for v in &vs {
                assert!(!v.identity.is_empty(), "{}: variant without identity", v.id);
                let entry = VARIANTS.iter().find(|e| e.0 == v.id).unwrap_or_else(|| panic!("{} missing from index.rs", v.id));
                assert_eq!(entry.1, *family, "{}: index family", v.id);
                assert_eq!(entry.2.to_vec(), v.identity, "{}: index identity", v.id);
            }
            all.extend(vs);
        }
        assert_eq!(VARIANTS.len(), all.len() - models().len(), "index.rs lists variants that don't exist");
        for m in &all {
            assert!(ids.insert(&m.id), "duplicate model id {}", m.id);
            assert!(!m.usb.is_empty(), "{}: no USB match", m.id);
            let codes: Vec<u16> = m.keys.values().map(|k| k.analog_code()).filter(|&c| c != 0).collect();
            let unique_codes: std::collections::HashSet<u16> = codes.iter().copied().collect();
            assert_eq!(codes.len(), unique_codes.len(), "{}: two keys report the same code", m.id);
            let drawn: Vec<u8> = m.layout.iter().map(|k| k.pos).collect();
            let unique: std::collections::BTreeSet<u8> = drawn.iter().copied().collect();
            assert_eq!(drawn.len(), unique.len(), "{}: a key is drawn twice", m.id);
            assert_eq!(unique, m.keys.keys().copied().collect(), "{}: drawn keys and key list differ", m.id);
            for k in &m.layout {
                assert!(k.x >= 0.0 && k.y >= 0.0 && k.w > 0.0 && k.h > 0.0, "{}: key {} has a bad size", m.id, k.pos);
                assert!(k.x + k.w <= m.width_units + 1e-9, "{}: key {} is past width_units", m.id, k.pos);
            }
            for (i, a) in m.layout.iter().enumerate() {
                if let Some([nw, ny]) = a.notch {
                    assert!(nw > 0.0 && nw < a.w && ny > 0.0 && ny < a.h, "{}: key {} has a bad notch", m.id, a.pos);
                }
                for b in &m.layout[i + 1..] {
                    for (ax, ay, aw, ah) in a.parts() {
                        for (bx, by, bw, bh) in b.parts() {
                            let overlap = ax < bx + bw - 1e-9 && bx < ax + aw - 1e-9 && ay < by + bh - 1e-9 && by < ay + ah - 1e-9;
                            assert!(!overlap, "{}: keys {} and {} overlap", m.id, a.pos, b.pos);
                        }
                    }
                }
            }
            let mut names = std::collections::HashSet::new();
            for k in m.keys.values() {
                assert!(names.insert(&k.key), "{}: key name {} used twice", m.id, k.key);
            }
        }
    }

    #[test]
    fn ak680_definition() {
        let m = model("ajazz-ak680-v2-he").unwrap();
        assert_eq!(m.keys.len(), 68);
        assert_eq!(m.position_of("KeyW"), Some(34));
        assert_eq!(m.keys[&85].key, "Fn");
        assert_eq!(m.keys[&85].hid, 0);
        assert!(m.matches(0x0C45, 0x80BC, 0xFF68, 0x61));
        assert!(m.owns_path(r"\\?\HID#VID_0C45&PID_80BC&MI_00#7&abc"));
        assert!(!m.owns_path(r"\\?\HID#VID_046D&PID_C52B"));
        assert_eq!(m.height_units, 5.0);
        let w = m.layout.iter().find(|k| k.pos == 34).unwrap();
        assert_eq!((w.x, w.y, w.w, w.h), (2.5, 1.0, 1.0, 1.0));
    }

    #[test]
    fn layout_gaps_and_tall_keys() {
        // F-row gap, a key moved down a quarter row, and a 2-unit tall numpad key
        let rows: Vec<Vec<RowKey>> = serde_json::from_str(
            r#"[[[1, "Esc", 1], {"pos": 2, "legend": "F1", "x": 0.5}],
                [{"pos": 3, "legend": "1", "y": 0.25}, {"pos": 4, "legend": "+", "h": 2}],
                [[5, "A", 1]]]"#,
        )
        .unwrap();
        let k = place(&rows);
        let at = |p: u8| k.iter().find(|r| r.pos == p).unwrap();
        assert_eq!((at(2).x, at(2).y), (1.5, 0.0));
        assert_eq!((at(3).x, at(3).y), (0.0, 1.25));
        assert_eq!((at(4).x, at(4).h), (1.0, 2.0));
        assert_eq!(at(5).y, 2.25); // the y offset carries to later rows
    }

    #[test]
    fn demo_is_separate_from_supported_models() {
        assert_eq!(model("demo-replay").unwrap().driver, "replay");
        assert!(models().iter().all(|m| m.id != "demo-replay"));
    }

    fn fake(id: &str) -> Model {
        let mut m = fallback().clone();
        m.id = id.into();
        m
    }

    #[test]
    fn picking_at_start() {
        let (a, b, f) = (fake("a"), fake("b"), fake("fallback"));
        assert_eq!(pick(&[], None, &f), (&f, false));
        assert_eq!(pick(&[&a], None, &f).0.id, "a");
        let (m, ask) = pick(&[&a, &b], None, &f);
        assert_eq!((m.id.as_str(), ask), ("a", true));
        // a default wins, connected or not, and never asks
        assert_eq!(pick(&[&a, &b], Some(&b), &f), (&b, false));
        assert_eq!(pick(&[], Some(&b), &f), (&b, false));
        assert_eq!(unique([&a, &b, &a]).len(), 2);
    }

    #[test]
    fn following_plug_and_unplug() {
        let (a, b, c) = (fake("a"), fake("b"), fake("c"));
        assert_eq!(follow(&a, &[&a, &b], false), Follow::Stay); // still here
        assert_eq!(follow(&a, &[&b], false), Follow::Switch(&b)); // gone, one other
        assert_eq!(follow(&a, &[&b, &c], false), Follow::Ask); // gone, several
        assert_eq!(follow(&a, &[], false), Follow::Stay); // nothing to switch to
        assert_eq!(follow(&a, &[&b], true), Follow::Stay); // the default is kept
    }
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

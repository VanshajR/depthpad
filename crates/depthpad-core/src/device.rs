//! Supported keyboard models. Each model is a data file in `data/devices/` that says how to find
//! the keyboard (USB IDs and the HID collection its driver talks to), which driver runs it, and
//! its keys: their positions (the driver's own key numbers), the HID usage each sends from the
//! factory, the `KeyboardEvent.code`-style name profiles use, and how to draw them.
//!
//! Adding a keyboard whose protocol is already implemented is just a new data file (see
//! CONTRIBUTING.md); a new protocol also needs a driver in `platform/drivers/`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize};

use crate::keymap::{KeyInfo, Keymap};

/// The bundled model definitions. Keep in sync with `data/devices/`.
const BUNDLED: &[&str] = &[
    include_str!("../data/devices/ajazz-ak680-v2-he.json"),
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
];

fn hex<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let s = String::deserialize(d)?;
    u16::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

fn hex_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u16>, D::Error> {
    hex(d).map(Some)
}

/// One HID collection that identifies the model. A missing product ID, usage page or usage
/// matches any (e.g. every Wooting keyboard has the same analog usage page).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsbMatch {
    #[serde(deserialize_with = "hex")]
    pub vid: u16,
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
        self.vid == vid
            && self.pid.is_none_or(|p| p == pid)
            && self.usage_page.is_none_or(|p| p == usage_page)
            && self.usage.is_none_or(|u| u == usage)
    }
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
/// row and the rows after it down, `w`/`h` are the size in key units.
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
}

/// Turn layout rows into key rectangles.
pub fn place(rows: &[Vec<RowKey>]) -> Vec<KeyRect> {
    let mut out = Vec::new();
    let mut y = 0.0;
    for row in rows {
        let mut x = 0.0;
        for k in row {
            let (pos, legend, w, h, dx, dy) = match k {
                RowKey::Plain(p, l, w) => (*p, l.clone(), *w, 1.0, 0.0, 0.0),
                RowKey::Full { pos, legend, w, h, x, y } => (*pos, legend.clone(), *w, *h, *x, *y),
            };
            y += dy;
            x += dx;
            out.push(KeyRect { pos, legend, x, y, w, h });
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
    pub fn owns_path(&self, path: &str) -> bool {
        let p = path.to_ascii_lowercase();
        self.usb.iter().any(|m| match m.pid {
            Some(pid) => p.contains(&format!("vid_{:04x}&pid_{:04x}", m.vid, pid)),
            None => p.contains(&format!("vid_{:04x}&", m.vid)),
        })
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
    models().iter().find(|m| m.id == id).or_else(|| (id == demo().id).then(demo))
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
    if has_default || connected.iter().any(|m| m.id == current.id) {
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
        for m in models() {
            assert!(ids.insert(&m.id), "duplicate model id {}", m.id);
            assert!(!m.usb.is_empty(), "{}: no USB match", m.id);
            let drawn: Vec<u8> = m.layout.iter().map(|k| k.pos).collect();
            let unique: std::collections::BTreeSet<u8> = drawn.iter().copied().collect();
            assert_eq!(drawn.len(), unique.len(), "{}: a key is drawn twice", m.id);
            assert_eq!(unique, m.keys.keys().copied().collect(), "{}: drawn keys and key list differ", m.id);
            for k in &m.layout {
                assert!(k.x >= 0.0 && k.y >= 0.0 && k.w > 0.0 && k.h > 0.0, "{}: key {} has a bad size", m.id, k.pos);
                assert!(k.x + k.w <= m.width_units + 1e-9, "{}: key {} is past width_units", m.id, k.pos);
            }
            for (i, a) in m.layout.iter().enumerate() {
                for b in &m.layout[i + 1..] {
                    let overlap = a.x < b.x + b.w - 1e-9 && b.x < a.x + a.w - 1e-9
                        && a.y < b.y + b.h - 1e-9 && b.y < a.y + a.h - 1e-9;
                    assert!(!overlap, "{}: keys {} and {} overlap", m.id, a.pos, b.pos);
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

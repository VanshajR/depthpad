//! Profiles: which keys drive which controller inputs, with per-binding response curves.
//! Stored as JSON (same format as the Python prototype), one file per profile.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Travel the curve tests assume (the AK680 V2 HE); real values come from the keyboard model.
#[cfg(test)]
const TEST_TRAVEL_MM: f64 = 3.4;

pub const STICK_TARGETS: &[&str] = &["LS_UP", "LS_DOWN", "LS_LEFT", "LS_RIGHT", "RS_UP", "RS_DOWN", "RS_LEFT", "RS_RIGHT"];
pub const TRIGGER_TARGETS: &[&str] = &["LT", "RT"];
pub const BUTTON_TARGETS: &[&str] = &[
    "A", "B", "X", "Y", "LB", "RB", "BACK", "START", "LS", "RS", "DPAD_UP", "DPAD_DOWN", "DPAD_LEFT", "DPAD_RIGHT",
    "GUIDE",
];

pub fn is_target(t: &str) -> bool {
    STICK_TARGETS.contains(&t) || TRIGGER_TARGETS.contains(&t) || BUTTON_TARGETS.contains(&t)
}
pub fn is_analog(t: &str) -> bool {
    STICK_TARGETS.contains(&t) || TRIGGER_TARGETS.contains(&t)
}

/// How key depth becomes output (Wootility's curve presets "Instant" and "Turbo" are modes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurveMode {
    /// Output follows the curve.
    #[default]
    Analog,
    /// Full output as soon as a key passes the dead zone; for sticks, the keys' depths still
    /// set the direction (Wootility's "Instant").
    Instant,
    /// On or off past the dead zone: sticks snap to the 8 directions (Wootility's "Turbo").
    Turbo,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Curve {
    pub deadzone_mm: f64,
    pub full_mm: f64,
    pub shape: f64,
    pub press_mm: f64,
    /// A hand-drawn curve: 4 points of (key travel mm, output 0..1), travel increasing. The
    /// first point's travel is the dead zone and the last one's is full output; beyond the
    /// last point the output stays at its value. None: dead zone, full output and shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<[[f64; 2]; 4]>,
    /// Output jumps to at least this (0..0.9) as soon as a key passes the dead zone, so a game's
    /// own stick dead zone doesn't swallow light presses.
    pub anti_deadzone: f64,
    pub mode: CurveMode,
}

impl Default for Curve {
    fn default() -> Self {
        Self { deadzone_mm: 0.2, full_mm: 2.6, shape: 1.0, press_mm: 1.0, points: None, anti_deadzone: 0.0, mode: CurveMode::Analog }
    }
}

/// Piecewise-linear output for `mm` along `points` (0 before the first point).
fn along(points: &[[f64; 2]; 4], mm: f64) -> f64 {
    if mm <= points[0][0] {
        return 0.0;
    }
    for w in points.windows(2) {
        let ([x0, y0], [x1, y1]) = (w[0], w[1]);
        if mm <= x1 {
            return if x1 - x0 < 1e-9 { y1 } else { y0 + (y1 - y0) * (mm - x0) / (x1 - x0) };
        }
    }
    points[3][1]
}

impl Curve {
    /// Output for a key at `depth` (0..1) of a stroke `travel_mm` long, before the stick-level
    /// part of the Instant mode (which the mapper applies, since it needs both axes).
    pub fn apply(&self, depth: f64, travel_mm: f64) -> f64 {
        let mm = depth * travel_mm;
        let v = match &self.points {
            Some(p) => along(p, mm).clamp(0.0, 1.0),
            None => {
                let dz = self.deadzone_mm;
                let full = self.full_mm.max(dz + 0.05);
                let v = ((mm - dz) / (full - dz)).clamp(0.0, 1.0);
                if v > 0.0 { v.powf(self.shape) } else { 0.0 }
            }
        };
        if v <= 0.0 && !(self.points.is_some() && mm > self.points.unwrap()[0][0]) {
            return 0.0;
        }
        match self.mode {
            CurveMode::Turbo => 1.0,
            _ => {
                let a = self.anti_deadzone.clamp(0.0, 0.9);
                a + (1.0 - a) * v
            }
        }
    }

    /// The built-in curve shapes (Wootility's presets), between this curve's dead zone and full
    /// output. "Instant" and "Turbo" are modes on a linear curve.
    pub fn preset(&self, name: &str) -> Option<Curve> {
        let (dz, full) = (self.deadzone_mm, self.full_mm.max(self.deadzone_mm + 0.2));
        let at = |f: f64| dz + (full - dz) * f;
        let points = |ys: [f64; 2], xs: [f64; 2]| Some([[dz, 0.0], [at(xs[0]), ys[0]], [at(xs[1]), ys[1]], [full, 1.0]]);
        let mut c = Curve { mode: CurveMode::Analog, ..*self };
        c.points = match name {
            "linear" | "instant" | "turbo" => points([1.0 / 3.0, 2.0 / 3.0], [1.0 / 3.0, 2.0 / 3.0]),
            "aggro" => points([0.6, 0.9], [0.25, 0.6]),
            "slow" => points([0.15, 0.45], [0.45, 0.8]),
            "smooth" => points([0.1, 0.9], [0.3, 0.7]),
            "steep" => points([0.05, 0.95], [0.4, 0.6]),
            _ => return None,
        };
        c.mode = match name {
            "instant" => CurveMode::Instant,
            "turbo" => CurveMode::Turbo,
            _ => CurveMode::Analog,
        };
        Some(c)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CurveOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadzone_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub press_mm: Option<f64>,
}

impl CurveOverride {
    pub fn is_custom(&self) -> bool {
        self.deadzone_mm.is_some() || self.full_mm.is_some() || self.shape.is_some() || self.press_mm.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub key: String,
    pub target: String,
    #[serde(flatten)]
    pub curve: CurveOverride,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub stick_shape: String,
    pub last_key_priority: bool,
    pub last_key_priority_sensitivity: f64,
    pub snappy_joystick: bool,
    pub block_keys: bool,
    pub block_allow_alt_tab: bool,
    /// Block every key while active, not only mapped ones (for keyboards used only as a pad).
    pub block_all_keys: bool,
    pub pause_hotkey: String,
    /// Depth assumed for a key the keyboard says is down but sends no analog data for (with
    /// another key held still, the AK680 only streams that one): "actuation" or "full".
    pub unread_depth: String,
    /// Where diagonals point, degrees from horizontal: 45 is even; higher leans toward up/down.
    pub diagonal_angle: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            stick_shape: "square".into(),
            last_key_priority: false,
            last_key_priority_sensitivity: 0.02,
            snappy_joystick: false,
            block_keys: false,
            block_allow_alt_tab: true,
            block_all_keys: false,
            pause_hotkey: "ControlRight+KeyP".into(),
            unread_depth: "actuation".into(),
            diagonal_angle: 45.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub curve: Curve,
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

impl Profile {
    /// The curve a binding uses: the profile's, with the key's own numbers if it has any (a key
    /// with its own response uses dead zone / full output / shape, not the drawn points).
    pub fn curve_of(&self, b: &Binding) -> Curve {
        let d = self.curve;
        let own_shape = b.curve.deadzone_mm.is_some() || b.curve.full_mm.is_some() || b.curve.shape.is_some();
        Curve {
            deadzone_mm: b.curve.deadzone_mm.unwrap_or(d.deadzone_mm),
            full_mm: b.curve.full_mm.unwrap_or(d.full_mm),
            shape: b.curve.shape.unwrap_or(d.shape),
            press_mm: b.curve.press_mm.unwrap_or(d.press_mm),
            points: if own_shape { None } else { d.points },
            ..d
        }
    }

    /// Problems that stop the profile being used; empty when valid.
    pub fn validate(&self) -> Vec<String> {
        let mut out = Vec::new();
        for b in &self.bindings {
            if !crate::device::known_key(&b.key) {
                out.push(format!("unknown key {:?}", b.key));
            }
            if !is_target(&b.target) {
                out.push(format!("unknown target {:?}", b.target));
            }
            let c = self.curve_of(b);
            let max = crate::device::max_travel_mm();
            if !(0.0 <= c.deadzone_mm && c.deadzone_mm < c.full_mm && c.full_mm <= max) {
                out.push(format!("{}: dead zone must be below full output, within 0-{max} mm", b.key));
            }
            if !(0.2..=5.0).contains(&c.shape) {
                out.push(format!("{}: shape must be 0.2-5", b.key));
            }
        }
        let c = &self.curve;
        if let Some(p) = &c.points {
            let ok = p.iter().all(|[mm, out]| mm.is_finite() && (0.0..=1.0).contains(out) && *mm >= 0.0)
                && p.windows(2).all(|w| w[1][0] >= w[0][0]);
            if !ok {
                out.push("curve points must have increasing travel and outputs from 0 to 1".into());
            }
        }
        if !(0.0..=0.9).contains(&c.anti_deadzone) {
            out.push("anti-deadzone must be 0-90%".into());
        }
        for part in self.settings.pause_hotkey.split('+').filter(|p| !p.is_empty()) {
            if !crate::device::known_key(part) {
                out.push(format!("pause hotkey: unknown key {part:?}"));
            }
        }
        out
    }
}

fn b(key: &str, target: &str) -> Binding {
    Binding { key: key.into(), target: target.into(), curve: CurveOverride::default() }
}

/// The two built-in layouts.
pub fn builtin(name: &str) -> Profile {
    let common = [
        ("ArrowUp", "RS_UP"), ("ArrowDown", "RS_DOWN"), ("ArrowLeft", "RS_LEFT"), ("ArrowRight", "RS_RIGHT"),
        ("Space", "A"), ("KeyC", "B"), ("KeyR", "X"), ("KeyG", "Y"), ("KeyQ", "LB"), ("KeyE", "RB"),
        ("Tab", "BACK"), ("Enter", "START"),
    ];
    let (name, movement, block): (&str, &[(&str, &str)], bool) = if name == "Racing" {
        ("Racing", &[("KeyW", "RT"), ("KeyS", "LT"), ("KeyA", "LS_LEFT"), ("KeyD", "LS_RIGHT")], true)
    } else {
        ("Default", &[("KeyW", "LS_UP"), ("KeyS", "LS_DOWN"), ("KeyA", "LS_LEFT"), ("KeyD", "LS_RIGHT"),
                      ("ShiftLeft", "LT"), ("KeyF", "RT")], false)
    };
    Profile {
        name: name.into(),
        settings: Settings { block_keys: block, ..Settings::default() },
        curve: Curve::default(),
        bindings: movement.iter().chain(common.iter()).map(|(k, t)| b(k, t)).collect(),
    }
}

/// Profiles as JSON files in a folder; the built-ins are written on first run.
pub struct ProfileStore {
    pub dir: PathBuf,
}

impl ProfileStore {
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let s = Self { dir: dir.into() };
        fs::create_dir_all(&s.dir)?;
        if s.names().is_empty() {
            s.save(&builtin("Default"))?;
            s.save(&builtin("Racing"))?;
        }
        Ok(s)
    }

    fn path(&self, name: &str) -> PathBuf {
        let safe: String = name
            .chars()
            .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
            .collect();
        let safe = safe.trim();
        self.dir.join(format!("{}.json", if safe.is_empty() { "Profile" } else { safe }))
    }

    pub fn load_file(path: &Path) -> io::Result<Profile> {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| Self::load_file(&e.path()).ok().map(|p| p.name))
            .collect();
        out.sort();
        out
    }

    pub fn load(&self, name: &str) -> io::Result<Profile> {
        Self::load_file(&self.path(name))
    }

    /// Atomic write (temp file + rename).
    pub fn save(&self, p: &Profile) -> io::Result<()> {
        let path = self.path(&p.name);
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(p).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?)?;
        fs::rename(tmp, path)
    }

    pub fn delete(&self, name: &str) -> io::Result<()> {
        match fs::remove_file(self.path(name)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }

    pub fn unique_name(&self, base: &str) -> String {
        let names = self.names();
        if !names.iter().any(|n| n == base) {
            return base.into();
        }
        (2..).map(|i| format!("{base} {i}")).find(|n| !names.contains(n)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_valid() {
        for n in ["Default", "Racing"] {
            assert!(builtin(n).validate().is_empty(), "{n}");
        }
    }

    #[test]
    fn json_compatible_with_prototype() {
        let json = r#"{"name": "X", "bindings": [{"key": "KeyW", "target": "RT", "full_mm": 1.5}]}"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.settings, Settings::default());
        assert_eq!(p.curve_of(&p.bindings[0]).full_mm, 1.5);
        assert_eq!(p.curve_of(&p.bindings[0]).deadzone_mm, 0.2);
        let back = serde_json::to_value(&p).unwrap();
        assert_eq!(back["bindings"][0]["full_mm"], 1.5);
        assert!(back["bindings"][0].get("shape").is_none());
    }

    #[test]
    fn curve() {
        let c = Curve::default();
        let t = TEST_TRAVEL_MM;
        assert_eq!(c.apply(0.0, t), 0.0);
        assert_eq!(c.apply(0.2 / t, t), 0.0);
        assert!((c.apply(1.4 / t, t) - 0.5).abs() < 1e-9);
        assert_eq!(c.apply(1.0, t), 1.0);
    }

    #[test]
    fn store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("depthpad-test-{}", std::process::id()));
        let s = ProfileStore::open(&dir).unwrap();
        assert_eq!(s.names(), vec!["Default", "Racing"]);
        let mut p = s.load("Racing").unwrap();
        p.name = s.unique_name("Racing");
        assert_eq!(p.name, "Racing 2");
        s.save(&p).unwrap();
        s.delete("Racing 2").unwrap();
        assert_eq!(s.names().len(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn curve_points_presets_and_modes() {
        let t = TEST_TRAVEL_MM;
        let base = Curve::default(); // dead zone 0.2, full 2.6
        let lin = base.preset("linear").unwrap();
        for mm in [0.2, 0.8, 1.4, 2.0, 2.6] {
            assert!((lin.apply(mm / t, t) - base.apply(mm / t, t)).abs() < 1e-9, "linear preset = plain curve at {mm}");
        }
        let aggro = base.preset("aggro").unwrap();
        assert!(aggro.apply(1.0 / t, t) > lin.apply(1.0 / t, t), "aggro gives more early");
        let slow = base.preset("slow").unwrap();
        assert!(slow.apply(1.0 / t, t) < lin.apply(1.0 / t, t), "slow gives less early");
        assert_eq!(aggro.apply(3.0 / t, t), 1.0);
        // anti-deadzone: a minimum once past the dead zone, nothing before it
        let anti = Curve { anti_deadzone: 0.25, ..lin };
        assert_eq!(anti.apply(0.1 / t, t), 0.0);
        assert!(anti.apply(0.21 / t, t) >= 0.25);
        // turbo: on/off past the dead zone
        let turbo = base.preset("turbo").unwrap();
        assert_eq!((turbo.apply(0.1 / t, t), turbo.apply(0.3 / t, t)), (0.0, 1.0));
        assert_eq!(base.preset("instant").unwrap().mode, CurveMode::Instant);
        assert!(base.preset("nope").is_none());
    }

    #[test]
    fn old_profiles_load_without_new_fields() {
        let p: Profile = serde_json::from_str(r#"{"name":"Old","curve":{"deadzone_mm":0.3,"full_mm":2.0,"shape":1.0,"press_mm":1.0},"bindings":[]}"#).unwrap();
        assert_eq!((p.curve.points, p.curve.anti_deadzone, p.curve.mode), (None, 0.0, CurveMode::Analog));
    }
}

//! Profile + key depths -> controller report.

use std::collections::HashMap;

use serde::Serialize;

use crate::fusion::Fusion;
use crate::device::Model;
use crate::profile::{Curve, CurveMode, Profile};
use crate::sticks::AxisResolver;

pub fn button_bit(target: &str) -> Option<u16> {
    Some(match target {
        "DPAD_UP" => 0x0001,
        "DPAD_DOWN" => 0x0002,
        "DPAD_LEFT" => 0x0004,
        "DPAD_RIGHT" => 0x0008,
        "START" => 0x0010,
        "BACK" => 0x0020,
        "LS" => 0x0040,
        "RS" => 0x0080,
        "LB" => 0x0100,
        "RB" => 0x0200,
        "GUIDE" => 0x0400,
        "A" => 0x1000,
        "B" => 0x2000,
        "X" => 0x4000,
        "Y" => 0x8000,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Report {
    pub buttons: u16,
    pub lt: f64,
    pub rt: f64,
    pub lx: f64,
    pub ly: f64,
    pub rx: f64,
    pub ry: f64,
}

pub struct Mapper {
    targets: HashMap<String, Vec<(u8, Curve)>>,
    circle: bool,
    resolvers: [AxisResolver; 4], // LS x, LS y, RS x, RS y
    pub positions: Vec<u8>,
    /// Full key travel of the model, mm (depth 1.0).
    travel_mm: f64,
    /// The profile's curve mode (Instant needs whole-stick handling).
    mode: CurveMode,
    /// Where an even diagonal points, degrees from horizontal (45 = unchanged).
    diagonal_angle: f64,
}

/// Bend a stick direction so an even diagonal (45 degrees) points at `center` degrees from
/// horizontal instead, keeping the magnitude: angles below 45 scale into 0..center, above into
/// center..90 (Wootility's "angle adjustment"). Pure up/down/left/right never change.
pub fn bend_diagonal(x: f64, y: f64, center: f64) -> (f64, f64) {
    if (center - 45.0).abs() < 1e-9 || x == 0.0 || y == 0.0 {
        return (x, y);
    }
    let r = x.hypot(y);
    let a = y.abs().atan2(x.abs()).to_degrees();
    let b = if a <= 45.0 { a * center / 45.0 } else { center + (a - 45.0) * (90.0 - center) / 45.0 };
    let (s, c) = b.to_radians().sin_cos();
    (r * c * x.signum(), r * s * y.signum())
}

impl Mapper {
    pub fn new(profile: &Profile, model: &Model) -> Self {
        let mut targets: HashMap<String, Vec<(u8, Curve)>> = HashMap::new();
        for b in &profile.bindings {
            if let Some(pos) = model.position_of(&b.key) {
                targets.entry(b.target.clone()).or_default().push((pos, profile.curve_of(b)));
            }
        }
        let mut positions: Vec<u8> = targets.values().flatten().map(|(p, _)| *p).collect();
        positions.sort_unstable();
        positions.dedup();
        let s = &profile.settings;
        let r = || AxisResolver::new(s.snappy_joystick, s.last_key_priority, s.last_key_priority_sensitivity);
        Self {
            targets,
            circle: s.stick_shape == "circle",
            resolvers: [r(), r(), r(), r()],
            positions,
            travel_mm: model.travel_mm,
            mode: profile.curve.mode,
            diagonal_angle: s.diagonal_angle.clamp(10.0, 80.0),
        }
    }

    /// Deepest curved value among the keys bound to a target (HallJoy's ReadSide).
    fn value(&self, f: &Fusion, target: &str, now: f64) -> f64 {
        self.targets
            .get(target)
            .map(|v| v.iter().map(|(p, c)| c.apply(f.depth(*p, now), self.travel_mm)).fold(0.0, f64::max))
            .unwrap_or(0.0)
    }

    fn pressed(&self, f: &Fusion, target: &str, now: f64) -> bool {
        self.targets
            .get(target)
            .is_some_and(|v| v.iter().any(|(p, c)| f.depth(*p, now) * self.travel_mm >= c.press_mm))
    }

    pub fn compute(&mut self, f: &Fusion, now: f64) -> Report {
        let mut axes = [0.0; 4];
        for (i, stick) in ["LS", "RS"].iter().enumerate() {
            let v = |d: &str| self.value(f, &format!("{stick}_{d}"), now);
            let (up, down, left, right) = (v("UP"), v("DOWN"), v("LEFT"), v("RIGHT"));
            let (mut x, mut y) = bend_diagonal(
                self.resolvers[i * 2].resolve(left, right),
                self.resolvers[i * 2 + 1].resolve(down, up),
                self.diagonal_angle,
            );
            if self.mode == CurveMode::Instant && (x != 0.0 || y != 0.0) {
                // full magnitude; the depths only choose the direction
                let m = x.abs().max(y.abs());
                (x, y) = (x / m, y / m);
            }
            let m = x.hypot(y);
            if self.circle && m > 1.0 {
                x /= m;
                y /= m;
            }
            axes[i * 2] = x;
            axes[i * 2 + 1] = y;
        }
        let mut buttons = 0;
        for t in self.targets.keys() {
            if let Some(bit) = button_bit(t) {
                if self.pressed(f, t, now) {
                    buttons |= bit;
                }
            }
        }
        let trigger = |v: f64| if self.mode == CurveMode::Instant && v > 0.0 { 1.0 } else { v };
        Report {
            buttons,
            lt: trigger(self.value(f, "LT", now)),
            rt: trigger(self.value(f, "RT", now)),
            lx: axes[0],
            ly: axes[1],
            rx: axes[2],
            ry: axes[3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bend_diagonal;

    fn angle(x: f64, y: f64) -> f64 {
        y.abs().atan2(x.abs()).to_degrees()
    }

    #[test]
    fn diagonal_bending() {
        // 45 leaves everything alone
        assert_eq!(bend_diagonal(0.7, 0.7, 45.0), (0.7, 0.7));
        // an even diagonal points where asked, magnitude kept, signs kept
        let (x, y) = bend_diagonal(-0.6, 0.6, 60.0);
        assert!((angle(x, y) - 60.0).abs() < 1e-9 && x < 0.0 && y > 0.0);
        assert!((x.hypot(y) - 0.6f64.hypot(0.6)).abs() < 1e-9);
        // straight directions never move
        assert_eq!(bend_diagonal(0.0, 1.0, 60.0), (0.0, 1.0));
        assert_eq!(bend_diagonal(1.0, 0.0, 30.0), (1.0, 0.0));
        // the bend is monotonic: a less vertical input stays less vertical
        let a = |x: f64, y: f64| { let (bx, by) = bend_diagonal(x, y, 60.0); angle(bx, by) };
        assert!(a(1.0, 0.2) < a(1.0, 0.5) && a(1.0, 0.5) < a(1.0, 1.0) && a(1.0, 1.0) < a(0.5, 1.0));
    }
}

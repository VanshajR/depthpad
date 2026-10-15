//! Combine the single-focus analog stream with the keyboard's own key events.
//!
//! Measured on 0C45:80BC (docs/PROTOCOL.md): the stream repeats only the most recently moved
//! key; a key that loses the stream is never reported again (its release is lost); a key
//! pressed while another, still, key keeps the stream may never be reported at all.
//!
//! Per key:
//! * stream packets for the key within `FRESH` -> the stream is exact, use it;
//! * otherwise key down (keyboard's HID reports) -> keep the last streamed depth; if the stream
//!   never reported this press, guess: the key's actuation point (it is at least that deep,
//!   since the keyboard reported it down), or full travel if `unread_full` is set;
//! * key-up of a key the stream wasn't following -> it is at its release point right now
//!   (learned from streamed keys, whose key-up arrives at a consistent depth): start there and
//!   ease to 0 over `RELEASE_RAMP` rather than jump;
//! * otherwise released.

use std::collections::HashMap;

pub const FRESH: f64 = 0.030;
pub const RELEASE_RAMP: f64 = 0.100;
pub const LEARN_WINDOW: f64 = 0.005;
pub const DEFAULT_STROKE: u16 = 34;
pub const DEFAULT_RELEASE: f64 = 145.0;
/// Guess for a key without a known actuation point (the AJAZZ configurator's default, 1.2 mm).
pub const DEFAULT_ACTUATION: f64 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Stream,
    Held,
    Release,
}

#[derive(Debug, Clone)]
struct KeyState {
    travel: f64,
    stream_time: f64,
    down: bool,
    stroke: u16,
    release: Option<f64>,
    ramp_from: f64,
    ramp_time: f64,
}

impl Default for KeyState {
    fn default() -> Self {
        Self {
            travel: 0.0,
            stream_time: f64::NEG_INFINITY,
            down: false,
            stroke: DEFAULT_STROKE,
            release: None,
            ramp_from: 0.0,
            ramp_time: f64::NEG_INFINITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fusion {
    keys: HashMap<u8, KeyState>,
    release: f64,
    pub focus: Option<u8>,
    /// Unread keys count as fully pressed (the old behaviour) instead of at their actuation point.
    pub unread_full: bool,
    /// Per-key actuation points, 0.01 mm.
    actuation: HashMap<u8, f64>,
}

impl Default for Fusion {
    fn default() -> Self {
        Self { keys: HashMap::new(), release: DEFAULT_RELEASE, focus: None, unread_full: true, actuation: HashMap::new() }
    }
}

impl Fusion {
    pub fn set_actuation(&mut self, actuation: impl IntoIterator<Item = (u8, f64)>) {
        self.actuation = actuation.into_iter().collect();
    }

    /// Depth (0.01 mm) to assume for a key that is down but never streamed this press.
    fn unread(&self, pos: u8, full: f64) -> f64 {
        if self.unread_full {
            full
        } else {
            self.actuation.get(&pos).copied().unwrap_or(DEFAULT_ACTUATION).min(full)
        }
    }

    pub fn on_stream(&mut self, pos: u8, travel: u16, stroke: u16, now: f64) {
        let k = self.keys.entry(pos).or_default();
        k.travel = travel as f64;
        k.stream_time = now;
        if (10..=50).contains(&stroke) {
            k.stroke = stroke;
        }
        self.focus = Some(pos);
    }

    pub fn on_digital(&mut self, pos: u8, down: bool, now: f64) {
        let k = self.keys.entry(pos).or_default();
        let was_down = k.down;
        k.down = down;
        if down || !was_down {
            return;
        }
        let full = k.stroke as f64 * 10.0;
        if now - k.stream_time <= LEARN_WINDOW && k.travel > 20.0 && k.travel < full - 20.0 {
            k.release = Some(match k.release {
                None => k.travel,
                Some(r) => (0.7 * r + 0.3 * k.travel).round(),
            });
            self.release = (0.8 * self.release + 0.2 * k.travel).round();
        }
        let point = k.release.unwrap_or(self.release);
        let guess = self.unread(pos, full);
        let k = self.keys.get_mut(&pos).expect("inserted above");
        k.ramp_from = if k.travel > 0.0 { point.min(k.travel) } else { point.min(guess) };
        k.ramp_time = now;
        if now - k.stream_time > FRESH {
            k.travel = 0.0;
        }
    }

    /// Stream silent or keyboard lost: forget all streamed depths.
    pub fn clear_stream(&mut self) {
        for k in self.keys.values_mut() {
            k.travel = 0.0;
            k.stream_time = f64::NEG_INFINITY;
            k.ramp_time = f64::NEG_INFINITY;
        }
    }

    /// Keyboard unplugged: forget key-down state too.
    pub fn release_all(&mut self) {
        for k in self.keys.values_mut() {
            k.down = false;
            k.travel = 0.0;
            k.ramp_time = f64::NEG_INFINITY;
        }
    }

    /// Key depth as a fraction of full travel, 0..1.
    pub fn depth(&self, pos: u8, now: f64) -> f64 {
        let Some(k) = self.keys.get(&pos) else { return 0.0 };
        let full = k.stroke as f64 * 10.0;
        let ramp = (!k.down && now - k.ramp_time < RELEASE_RAMP)
            .then(|| k.ramp_from * (1.0 - (now - k.ramp_time) / RELEASE_RAMP));
        let travel = if now - k.stream_time <= FRESH {
            // after a key-up the key is at most at its release point, even if the stream's last
            // (possibly stale) packet for it says deeper
            ramp.map_or(k.travel, |r| k.travel.min(r))
        } else if k.down {
            if k.travel > 0.0 { k.travel } else { self.unread(pos, full) }
        } else {
            ramp.unwrap_or(0.0)
        };
        (travel / full).min(1.0)
    }

    pub fn source(&self, pos: u8, now: f64) -> Option<Source> {
        let k = self.keys.get(&pos)?;
        if now - k.stream_time <= FRESH {
            Some(Source::Stream)
        } else if k.down {
            Some(Source::Held)
        } else if now - k.ramp_time < RELEASE_RAMP {
            Some(Source::Release)
        } else {
            None
        }
    }

    pub fn positions(&self) -> impl Iterator<Item = u8> + '_ {
        self.keys.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unread_key_guess() {
        // W streams and is held still; D goes down without any analog data (the AK680 keeps
        // following W)
        let (w, d) = (34u8, 51u8);
        let mut f = Fusion { unread_full: false, ..Fusion::default() };
        f.set_actuation([(d, 80.0)]);
        f.on_stream(w, 200, 34, 0.0);
        f.on_digital(w, true, 0.0);
        f.on_digital(d, true, 0.5);
        assert!((f.depth(d, 0.5) - 80.0 / 340.0).abs() < 1e-9); // its actuation point, not full
        assert_eq!(f.source(d, 0.5), Some(Source::Held));
        // released: eases down from the guess, never jumps up to the release point
        f.on_digital(d, false, 1.0);
        assert!(f.depth(d, 1.0) <= 80.0 / 340.0 + 1e-9);
        assert_eq!(f.depth(d, 1.2), 0.0);
        // no actuation known for a key: the default guess
        f.on_digital(52, true, 2.0);
        assert!((f.depth(52, 2.0) - DEFAULT_ACTUATION / 340.0).abs() < 1e-9);
        // the old behaviour on request
        f.unread_full = true;
        assert_eq!(f.depth(52, 2.0), 1.0);
    }
}

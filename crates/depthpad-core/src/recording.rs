//! Reading recorded sessions, for the replay (demo) keyboard and for tests. Two formats:
//!
//! - Depthpad session logs (About > Record a session): `<ms> S <pos> <travel>` analog samples,
//!   `<ms> K <pos> <0|1>` key events; other lines (output, events) are ignored.
//! - Early capture files: `<ms> key= <pos> st=.. adc=<adc> travel=<travel>` samples and
//!   `<ms> DIGITAL <NAME> down|up` key events with names like `W`, `SPACE`, `SHIFT`. They predate
//!   stroke logging, so the firmware's zeroed glitch packets (a lone adc=0/travel=0 right after a
//!   deep reading, dropped live by stroke = 0) are dropped here.

use std::collections::HashMap;

use crate::device::Model;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// Seconds from the start of the recording; travel in 0.01 mm.
    Sample { t: f64, pos: u8, travel: u16 },
    Key { t: f64, pos: u8, down: bool },
}

impl Event {
    pub fn t(&self) -> f64 {
        match self {
            Event::Sample { t, .. } | Event::Key { t, .. } => *t,
        }
    }
}

/// Capture-file key names -> profile key names.
fn capture_key(name: &str) -> String {
    match name {
        "SPACE" => "Space".into(),
        "SHIFT" => "ShiftLeft".into(),
        "CTRL" => "ControlLeft".into(),
        n if n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic()) => format!("Key{n}"),
        n => n.into(),
    }
}

/// Parse a recording; positions are the model's. Unreadable lines are skipped.
pub fn parse(text: &str, model: &Model) -> Vec<Event> {
    let mut out = Vec::new();
    let mut last: HashMap<u8, u16> = HashMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(ms) = parts[0].parse::<f64>() else { continue };
        let t = ms / 1000.0;
        match parts[1] {
            "S" => {
                if let (Ok(pos), Some(Ok(travel))) = (parts[2].parse(), parts.get(3).map(|p| p.parse())) {
                    out.push(Event::Sample { t, pos, travel });
                }
            }
            "K" => {
                if let (Ok(pos), Some(down)) = (parts[2].parse(), parts.get(3)) {
                    out.push(Event::Key { t, pos, down: *down == "1" });
                }
            }
            "DIGITAL" => {
                if let (Some(pos), Some(state)) = (model.position_of(&capture_key(parts[2])), parts.get(3)) {
                    out.push(Event::Key { t, pos, down: *state == "down" });
                }
            }
            first => {
                let Some(k) = first.strip_prefix("key=") else { continue };
                let k = if k.is_empty() { parts[2] } else { k };
                let field = |name: &str| parts.iter().find_map(|p| p.strip_prefix(name)).and_then(|v| v.parse::<u16>().ok());
                let (Ok(pos), Some(adc), Some(travel)) = (k.parse::<u8>(), field("adc="), field("travel=")) else { continue };
                if adc == 0 && travel == 0 && last.get(&pos).copied().unwrap_or(0) > 50 {
                    continue; // glitch packet
                }
                last.insert(pos, travel);
                out.push(Event::Sample { t, pos, travel });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::fallback;

    #[test]
    fn both_formats() {
        let m = fallback();
        let session = "  12.0 S 34 150\n  13.0 K 34 1\n  14.0 O 0 0 0 0 0 0 0\n  15.0 E paused\n  16.0 K 34 0\n";
        assert_eq!(parse(session, m), vec![
            Event::Sample { t: 0.012, pos: 34, travel: 150 },
            Event::Key { t: 0.013, pos: 34, down: true },
            Event::Key { t: 0.016, pos: 34, down: false },
        ]);
        let capture = "  1.0 key= 34 st=0 adc=900 travel=200\n  2.0 key= 34 st=0 adc=0 travel=0\n  3.0 DIGITAL W down\n  4.0 DIGITAL SPACE up\n";
        assert_eq!(parse(capture, m), vec![
            Event::Sample { t: 0.001, pos: 34, travel: 200 },
            Event::Key { t: 0.003, pos: 34, down: true },
            Event::Key { t: 0.004, pos: 83, down: false },
        ]);
    }

    #[test]
    fn bundled_capture_parses() {
        let events = parse(include_str!("../tests/data/capture_chords.log"), fallback());
        assert!(events.len() > 50_000);
        assert_eq!(events.iter().filter(|e| matches!(e, Event::Key { .. })).count(), 50);
        assert!(events.windows(2).all(|w| w[0].t() <= w[1].t() + 1e-9));
    }
}

//! Replay a real capture (stream + ground-truth key events) through Fusion.
//!
//! capture_chords.log was recorded on an AK680 V2 (0C45:80BC, fw 11.01) with GetAsyncKeyState
//! as ground truth. Events before 20 s came from the laptop's own keyboard and are skipped. The
//! log predates stroke logging, so the firmware's zeroed glitch packets (dropped live by
//! stroke = 0) are dropped here as a lone adc=0/travel=0 right after a deep reading.

use std::collections::HashMap;

use depthpad_core::fusion::Fusion;

const POS: [(&str, u8); 6] = [("W", 34), ("A", 49), ("S", 50), ("D", 51), ("SPACE", 83), ("SHIFT", 64)];

fn replay(checkpoints: &[f64]) -> HashMap<u64, HashMap<&'static str, f64>> {
    let log = include_str!("data/capture_chords.log");
    let mut f = Fusion::default();
    let mut out = HashMap::new();
    let mut pending: Vec<f64> = checkpoints.to_vec();
    pending.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut last: HashMap<u8, u16> = HashMap::new();
    for line in log.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(ms) = parts[0].parse::<f64>() else { continue };
        let t = ms / 1000.0;
        while !pending.is_empty() && pending[0] <= t {
            let c = pending.remove(0);
            out.insert((c * 1000.0) as u64, POS.iter().map(|(n, p)| (*n, (f.depth(*p, c) * 100.0).round() / 100.0)).collect());
        }
        if t < 20.0 {
            continue;
        }
        if parts[1] == "DIGITAL" {
            let pos = POS.iter().find(|(n, _)| *n == parts[2]).map(|(_, p)| *p).unwrap();
            f.on_digital(pos, parts[3] == "down", t);
        } else if let Some(k) = parts[1].strip_prefix("key=") {
            let k = if k.is_empty() { parts[2] } else { k };
            let pos: u8 = k.parse().unwrap();
            let field = |name: &str| -> u16 {
                parts.iter().find_map(|p| p.strip_prefix(name)).unwrap().parse().unwrap()
            };
            let (adc, travel) = (field("adc="), field("travel="));
            if adc == 0 && travel == 0 && last.get(&pos).copied().unwrap_or(0) > 50 {
                continue;
            }
            last.insert(pos, travel);
            f.on_stream(pos, travel, 34, t);
        }
    }
    out
}

fn at<'a>(r: &'a HashMap<u64, HashMap<&'static str, f64>>, t: f64) -> &'a HashMap<&'static str, f64> {
    &r[&((t * 1000.0) as u64)]
}

#[test]
fn chords_from_real_capture() {
    let r = replay(&[26.0, 28.5, 30.5, 32.5, 34.0, 36.0, 46.0, 49.0]);
    assert!(at(&r, 26.0)["W"] > 0.5);
    assert!(at(&r, 28.5)["W"] > 0.5 && at(&r, 28.5)["D"] > 0.9);
    // W released while D held: the stream never sent this release
    assert_eq!(at(&r, 30.5)["W"], 0.0);
    assert!(at(&r, 30.5)["D"] > 0.9);
    assert_eq!(at(&r, 32.5)["D"], 0.0);
    // A held, S not pressed yet
    assert!(at(&r, 34.0)["A"] > 0.9 && at(&r, 34.0)["S"] == 0.0);
    assert_eq!(at(&r, 36.0)["S"], 0.0);
    assert!(at(&r, 46.0)["W"] > 0.5 && at(&r, 46.0)["SPACE"] > 0.5);
    assert_eq!(at(&r, 49.0)["W"], 0.0);
    assert_eq!(at(&r, 49.0)["SPACE"], 0.0);
}

#[test]
fn s_taps_register_while_a_is_held() {
    // S down at 35.163-35.289, 37.440-37.560, 39.694-39.778; the stream never reported S
    let r = replay(&[35.2, 37.5, 39.75]);
    for (t, v) in &r {
        assert!(v["S"] == 1.0 && v["A"] > 0.9, "{t}: {v:?}");
    }
}

//! Read-only probe of GET_KEY_STATUS (0x68): polls W, A, S, D for a few seconds and prints their
//! travel whenever it changes, plus how many polls per second the keyboard answers. Changes
//! nothing on the keyboard (no calibration, no settings). Close Depthpad first.
//!   cargo run -p depthpad-core --example keystatus -- [seconds]
use std::collections::HashMap;
use std::time::Instant;

use depthpad_core::device::fallback;
use depthpad_core::platform::drivers::sonix_ajazz::Keyboard;
use depthpad_core::protocols::sonix_ajazz::{key_status_data, parse_key_status, GET_KEY_STATUS};

fn main() {
    let secs: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let model = fallback();
    let keys: Vec<u8> = ["KeyW", "KeyA", "KeyS", "KeyD"].iter().filter_map(|k| model.position_of(k)).collect();
    let mut kb = Keyboard::open(model).expect("open keyboard (is Depthpad still running?)");
    println!("firmware {}; polling W A S D for {secs} s; press them slowly, together too", kb.info.version);
    let data = key_status_data(&keys);
    let (start, mut polls, mut answered) = (Instant::now(), 0u32, 0u32);
    let mut last: HashMap<u8, u16> = HashMap::new();
    let mut first_reply = true;
    while start.elapsed().as_secs_f64() < secs {
        polls += 1;
        let reply = kb.command_data(GET_KEY_STATUS, data.len() as u8, 0, true, &data, 0.05).expect("io");
        let Some(r) = reply else { continue };
        answered += 1;
        if first_reply {
            first_reply = false;
            println!("first reply: {:02X?}", &r[..r.len().min(48)]);
        }
        for s in parse_key_status(&r, &keys) {
            let prev = last.insert(s.position, s.travel);
            if prev.map_or(true, |p| p.abs_diff(s.travel) >= 5) {
                println!("{:7.3}s key {:3} travel {:4} (x0.01 mm) adc {:5} cal {}",
                         start.elapsed().as_secs_f64(), s.position, s.travel, s.adc, s.calibration);
            } else {
                last.insert(s.position, prev.unwrap());
            }
        }
    }
    let t = start.elapsed().as_secs_f64();
    println!("{polls} polls, {answered} answered in {t:.1} s = {:.0} answers/s", answered as f64 / t);
}

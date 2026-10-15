//! Probe: does restarting the analog stream (off, then on) make the keyboard follow a different
//! key? Runs the stream for a while, restarting it every 0.5 s, and prints which keys streamed in
//! each half second. Hold W still, then press and move D: if D shows up after a restart while W
//! is still held, restarting is a way to follow the second key. Only switches the stream off and
//! on (as Depthpad does when it connects). Close Depthpad first.
//!   cargo run -p depthpad-core --example refocus -- [seconds]
use std::collections::BTreeMap;
use std::time::Instant;

use depthpad_core::device::fallback;
use depthpad_core::platform::drivers::sonix_ajazz::Keyboard;
use depthpad_core::protocols::sonix_ajazz::parse_stream;

fn main() {
    let secs: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(15.0);
    let model = fallback();
    let name = |p: u8| model.keys.get(&p).map_or(format!("{p}"), |k| k.key.trim_start_matches("Key").to_string());
    let mut kb = Keyboard::open(model).expect("open keyboard (is Depthpad still running?)");
    kb.start_stream().expect("start");
    let start = Instant::now();
    let mut window_start = start;
    let mut seen: BTreeMap<u8, (u32, u16)> = BTreeMap::new();
    let mut restarts = 0;
    while start.elapsed().as_secs_f64() < secs {
        if let Ok(Some(p)) = kb.read(5) {
            if let Some(s) = parse_stream(p) {
                let e = seen.entry(s.position).or_insert((0, 0));
                e.0 += 1;
                e.1 = s.travel;
            }
        }
        if window_start.elapsed().as_secs_f64() >= 0.5 {
            let summary: Vec<String> = seen.iter().map(|(p, (n, t))| format!("{}:{}x@{:.2}mm", name(*p), n, *t as f64 / 100.0)).collect();
            println!("{:5.1}s {}", start.elapsed().as_secs_f64(), if summary.is_empty() { "-".into() } else { summary.join("  ") });
            seen.clear();
            let off = kb.stop_stream().unwrap_or(false);
            let on = kb.start_stream().unwrap_or(false);
            restarts += 1;
            if !(off && on) {
                println!("      restart acknowledged: off={off} on={on}");
            }
            window_start = Instant::now();
        }
    }
    let _ = kb.stop_stream();
    println!("{restarts} restarts");
}

//! Hardware smoke test: run the engine for a few seconds and print what it sees.
//!   cargo run -p depthpad-core --example selftest -- [seconds]
use std::time::Duration;

use depthpad_core::device;
use depthpad_core::engine::Engine;
use depthpad_core::platform::drivers::detect;
use depthpad_core::profile::builtin;

fn main() {
    let secs: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let found = detect();
    for f in &found {
        println!("found {} ({}) at {}", f.model.name, f.model.driver, f.path);
    }
    let model = found.first().map_or_else(device::fallback, |f| f.model);
    let mut engine = Engine::start(model, builtin("Default"), None, true);
    std::thread::sleep(Duration::from_secs_f64((secs - 0.5).max(0.5)));
    engine.snapshot();
    std::thread::sleep(Duration::from_millis(500));
    let s = engine.snapshot();
    println!(
        "state={:?} rate={:.0}/s firmware={:?} pad={} pad_error={:?} blocking={} active_keys={}",
        s.state, s.rate, s.firmware, s.pad, s.pad_error, s.blocking, s.keys.len()
    );
    engine.stop(); // stream off (acknowledged), controller unplugged
    for e in engine.events() {
        println!("event: {e}");
    }
}

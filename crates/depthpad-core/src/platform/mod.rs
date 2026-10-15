//! Windows plumbing: keyboard drivers and HID access, per-device key events (Raw Input), the
//! optional key-blocking hook and the virtual Xbox 360 controller (ViGEmBus).

pub mod hid;
pub mod hook;
pub mod overlay;
pub mod drivers;
pub mod pad;
pub mod rawinput;
pub mod scancodes;
pub mod sound;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

/// Seconds since the process started, from the high-resolution counter (QPC). Every thread
/// stamps its events with this clock so the fusion's 5-30 ms windows mean the same thing.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// When Raw Input last saw a physical key event from any keyboard (seconds, `now()` clock).
/// The key hook's watchdog compares it with the hook's own last event: Windows calls the hook
/// before Raw Input, so Raw Input hearing keys the hook didn't means Windows removed the hook.
static LAST_RAW_KEY: AtomicU64 = AtomicU64::new(0);

pub(crate) fn note_raw_key(t: f64) {
    LAST_RAW_KEY.store(t.to_bits(), Ordering::Relaxed);
}

pub(crate) fn last_raw_key() -> f64 {
    f64::from_bits(LAST_RAW_KEY.load(Ordering::Relaxed))
}

/// UTF-16, NUL-terminated.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

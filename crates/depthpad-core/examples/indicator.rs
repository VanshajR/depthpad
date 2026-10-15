//! Shows the switch indicator once: the on-screen badge and the sound cue.
//!   cargo run -p depthpad-core --example indicator -- [title] [detail]
use depthpad_core::platform::{overlay, sound};

fn main() {
    // like the app: DPI-aware, so the badge must scale itself
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let mut args = std::env::args().skip(1);
    let title = args.next().unwrap_or_else(|| "Racing".into());
    let detail = args.next().unwrap_or_else(|| "Profile 2 of 2".into());
    overlay::show(&title, &detail);
    sound::play(sound::Cue::Profile(2));
    std::thread::sleep(std::time::Duration::from_millis(2200));
}

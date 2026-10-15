//! Plugs in each virtual controller kind for a moment and unplugs it.
//!   cargo run -p depthpad-core --example padcheck
use depthpad_core::mapper::Report;
use depthpad_core::platform::pad::{Pad, PadKind};

fn main() {
    for kind in [PadKind::Xbox, PadKind::Ds4] {
        match Pad::connect(kind) {
            Ok(mut p) => {
                p.update(&Report { lx: 0.5, ..Report::default() });
                std::thread::sleep(std::time::Duration::from_millis(300));
                println!("{kind:?}: plugged in, report sent, unplugging");
            }
            Err(e) => println!("{kind:?}: {e}"),
        }
    }
}

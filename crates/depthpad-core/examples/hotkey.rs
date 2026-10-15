//! Pause-hotkey check without a person at the keyboard: starts the engine, sends Right Ctrl+P as
//! synthetic input (accepted because this sets DEPTHPAD_HOOK_ACCEPT_INJECTED), and reports
//! whether the engine paused and resumed.
//!   cargo run -p depthpad-core --example hotkey
use std::time::Duration;

use depthpad_core::device;
use depthpad_core::engine::Engine;
use depthpad_core::profile::builtin;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, VIRTUAL_KEY,
};

fn key(scan: u16, extended: bool, up: bool) {
    let mut flags = KEYEVENTF_SCANCODE;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: scan, dwFlags: KEYBD_EVENT_FLAGS(flags.0), time: 0, dwExtraInfo: 0 } },
    };
    unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(Duration::from_millis(50));
}

fn combo() {
    key(0x1D, true, false); // Right Ctrl down
    key(0x19, false, false); // P down
    key(0x19, false, true);
    key(0x1D, true, true);
}

fn main() {
    // `hotkey send`: only press Right Ctrl+P (to test another process, e.g. the app)
    if std::env::args().nth(1).as_deref() == Some("send") {
        combo();
        return;
    }
    unsafe { std::env::set_var("DEPTHPAD_HOOK_ACCEPT_INJECTED", "1") };
    let mut engine = Engine::start(device::fallback(), builtin("Default"), None, false);
    std::thread::sleep(Duration::from_millis(500));
    println!("paused at start: {}", engine.paused());
    combo();
    std::thread::sleep(Duration::from_millis(200));
    println!("after Right Ctrl+P: {}", engine.paused());
    combo();
    std::thread::sleep(Duration::from_millis(200));
    println!("after it again: {}", engine.paused());
    engine.stop();
    for e in engine.events() {
        println!("event: {e}");
    }
}

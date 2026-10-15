//! Key descriptions shared by every keyboard model (see `device`).

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyInfo {
    /// HID keyboard usage the key sends from the factory (0 = handled inside the keyboard, e.g. Fn).
    pub hid: u16,
    /// `KeyboardEvent.code`-style name, used in profiles (e.g. "KeyW", "ShiftLeft").
    pub key: String,
    /// The code an analog protocol reports this key's depth under, when it isn't `hid`: keys
    /// without a USB usage (Fn, layer and vendor keys) use AnalogSense's extended codes (0x400+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
}

impl KeyInfo {
    /// What an analog protocol calls this key (0 = it doesn't report it).
    pub fn analog_code(&self) -> u16 {
        self.code.unwrap_or(self.hid)
    }
}

pub type Keymap = BTreeMap<u8, KeyInfo>;

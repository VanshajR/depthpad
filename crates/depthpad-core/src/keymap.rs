//! Key descriptions shared by every keyboard model (see `device`).

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyInfo {
    /// HID keyboard usage the key sends from the factory (0 = handled inside the keyboard, e.g. Fn).
    pub hid: u16,
    /// `KeyboardEvent.code`-style name, used in profiles (e.g. "KeyW", "ShiftLeft").
    pub key: String,
}

pub type Keymap = BTreeMap<u8, KeyInfo>;

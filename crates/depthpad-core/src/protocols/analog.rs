//! What every analog keyboard protocol looks like to the analog driver
//! (`platform::drivers::analog`): a pure state machine that says what to send to the keyboard
//! and turns what it sends back into the depth of each pressed key. No I/O happens here.
//!
//! Reports are given without their report ID, which is passed separately. Keys are reported
//! under their analog code: the HID usage for ordinary keys, AnalogSense's extended codes
//! (0x400+) for keys without one (see `keymap::KeyInfo::code`).

use std::collections::BTreeMap;

/// How a report goes to the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutKind {
    /// An output report (interrupt write, falling back to the control channel).
    Write,
    /// A feature report.
    Feature,
    /// Ask for a feature report this long (data bytes); the reply goes to `feature`.
    GetFeature(usize),
    /// Wait this many milliseconds before the next report (firmware that needs gaps).
    Pause(u32),
}

/// A report to send: report ID and data (without the ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    pub report_id: u8,
    pub data: Vec<u8>,
    pub kind: OutKind,
}

impl Out {
    /// An output report of `len` data bytes starting with `head`, zero-padded.
    pub fn write(report_id: u8, len: usize, head: &[u8]) -> Out {
        let mut data = vec![0u8; len.max(head.len())];
        data[..head.len()].copy_from_slice(head);
        Out { report_id, data, kind: OutKind::Write }
    }
    /// A feature report of `len` data bytes starting with `head`, zero-padded.
    pub fn feature(report_id: u8, len: usize, head: &[u8]) -> Out {
        Out { kind: OutKind::Feature, ..Out::write(report_id, len, head) }
    }
    /// A request for a feature report.
    pub fn get_feature(report_id: u8, len: usize) -> Out {
        Out { report_id, data: Vec::new(), kind: OutKind::GetFeature(len) }
    }
    /// A gap before the next report.
    pub fn pause(ms: u32) -> Out {
        Out { report_id: 0, data: Vec::new(), kind: OutKind::Pause(ms) }
    }
}

/// Pressed keys: analog code -> depth 0..1.
pub type Keys = BTreeMap<u16, f64>;

pub trait AnalogProtocol: Send {
    /// Reports to send right after the keyboard is opened (and again when it goes silent).
    fn start(&mut self) -> Vec<Out> {
        Vec::new()
    }
    /// An input report arrived. Returns the complete set of pressed keys when the report
    /// completes one, and what to send next.
    fn input(&mut self, report_id: u8, data: &[u8]) -> (Option<Keys>, Vec<Out>);
    /// A requested feature report arrived (see `OutKind::GetFeature`).
    fn feature(&mut self, report_id: u8, data: &[u8]) -> (Option<Keys>, Vec<Out>) {
        self.input(report_id, data)
    }
    /// Called about every 10 ms; for protocols that poll, retry or keep the keyboard awake.
    fn tick(&mut self, _now: f64) -> Vec<Out> {
        Vec::new()
    }
    /// What the keyboard's firmware calls itself, once known: picks the family variant
    /// (see `device::variant`).
    fn identity(&self) -> Option<String> {
        None
    }
    /// A reason this keyboard can't be used (e.g. firmware the protocol wasn't written for).
    /// The driver shows it and stops talking to the keyboard until it is plugged in again.
    fn problem(&self) -> Option<String> {
        None
    }
    /// Input reports come from another collection of the same keyboard (usage page, usage):
    /// the opened one only takes requests.
    fn input_collection(&self) -> Option<(u16, u16)> {
        None
    }
    /// How long to wait for an input report each round, or None when the protocol only polls
    /// (with feature reports from `tick`) and nothing arrives unasked.
    fn read_timeout_ms(&self) -> Option<u32> {
        Some(5)
    }
    /// Send output reports over the control channel only.
    fn control_channel(&self) -> bool {
        false
    }
    /// The keyboard only reports changes, so silence is normal while this is true (otherwise
    /// a silent keyboard counts as lost and the protocol is started again).
    fn event_driven(&self) -> bool {
        false
    }
    /// Reports to send before letting go of the keyboard (e.g. switch its analog stream off).
    fn stop(&mut self) -> Vec<Out> {
        Vec::new()
    }
}

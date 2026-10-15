//! Keyboard drivers. A driver finds its keyboard, runs the analog session on its own thread
//! (connect, start the stream, recover from silence and unplugging, stop the stream on exit)
//! and reports samples to the engine through a `StreamSink`. Everything after that (fusion,
//! sticks, mapping, blocking, the virtual controller) is shared by every keyboard.
//!
//! To add a protocol family: a pure parser in `protocols/`, a driver module here implementing
//! `Session`, an arm in `start`, and a model file in `data/devices/` naming the driver.

pub mod analog;
pub mod replay;
pub mod sonix_ajazz;

use std::collections::BTreeMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;

use crate::device::{demo, models, Model};
use crate::protocols::Sample;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamState {
    /// No keyboard open (not connected, or another program owns it).
    Searching,
    Streaming,
    /// Connected, but the keyboard did not confirm the stream started.
    NoAck,
    /// Connected, but no analog data recently.
    Silent,
}

/// What a driver learned on connecting.
pub struct Connected {
    pub firmware: Option<String>,
    /// Each key position -> the HID usage it sends now (keyboard-side remaps applied).
    pub to_usage: BTreeMap<u8, u16>,
    /// Anything worth telling the user about the key layout.
    pub note: Option<String>,
    /// Each key's actuation point (0.01 mm) where the keyboard reports it: the best guess for
    /// the depth of a key that is down but that the analog data doesn't cover.
    pub actuation: BTreeMap<u8, f64>,
}

/// Callbacks from a driver's thread.
pub trait StreamSink: Send + Sync + 'static {
    fn sample(&self, s: Sample, t: f64);
    /// Analog readings can no longer be trusted (stream silent, keyboard lost).
    fn lost(&self);
    fn connected(&self, c: Connected);
    fn event(&self, text: String);
    /// A key going down or up, for drivers that report key state themselves. Physical
    /// keyboards don't need this: their key events come from Raw Input.
    fn key(&self, _pos: u8, _down: bool, _t: f64) {}
    /// The keyboard turned out to be this variant of the model being run (see
    /// `device::variant`): the app switches to it.
    fn identified(&self, _variant: &'static crate::device::Model) {}
}

pub struct StreamStatus {
    pub state: Mutex<StreamState>,
    pub error: Mutex<Option<String>>,
    pub packets: AtomicU64,
    pub dropped: AtomicU64,
    /// Times the stream had to be restarted (another program switching it off shows up here).
    pub rearms: AtomicU64,
}

impl StreamStatus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(StreamState::Searching),
            error: Mutex::new(None),
            packets: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            rearms: AtomicU64::new(0),
        })
    }
}

/// A running driver.
pub trait Session: Send {
    fn status(&self) -> &Arc<StreamStatus>;
    /// Stop the analog stream and the driver thread.
    fn stop(&mut self);
}

/// A supported keyboard that is connected now.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub model: &'static Model,
    pub path: String,
    pub serial: Option<String>,
}

/// Connected keyboards that a bundled model describes, one entry per keyboard.
pub fn detect() -> Vec<Found> {
    let Ok(api) = hidapi::HidApi::new() else { return Vec::new() };
    let mut out: Vec<Found> = Vec::new();
    for d in api.device_list() {
        let path = d.path().to_string_lossy().into_owned();
        let Some(model) = models().iter().find(|m| {
            m.usb_match(d.vendor_id(), d.product_id(), d.usage_page(), d.usage())
                .is_some_and(|u| u.report_id.is_none_or(|id| crate::platform::hid::input_report_ids(&path).contains(&id)))
        }) else {
            continue;
        };
        let serial = d.serial_number().filter(|s| !s.is_empty()).map(str::to_owned);
        if !out.iter().any(|f| f.path == path) {
            out.push(Found { model, path, serial });
        }
    }
    if let Some(file) = replay::configured() {
        out.push(Found { model: demo(), path: format!("replay:{file}"), serial: None });
    }
    out
}

/// Start the driver for a model. The session keeps looking for the keyboard until it appears.
pub fn start(model: &'static Model, sink: Arc<dyn StreamSink>) -> Box<dyn Session> {
    match model.driver.as_str() {
        "sonix-ajazz" => Box::new(sonix_ajazz::StreamReader::start(model, sink)),
        "replay" => Box::new(replay::Replay::start(model, sink)),
        d if crate::protocols::analog_protocol(d, &Default::default()).is_some() => Box::new(analog::AnalogReader::start(model, sink)),
        other => Box::new(Unsupported::new(format!("{} needs the \"{other}\" driver, which this version doesn't have.", model.name))),
    }
}

/// A model file naming a driver this build doesn't include.
struct Unsupported(Arc<StreamStatus>);

impl Unsupported {
    fn new(msg: String) -> Self {
        let status = StreamStatus::new();
        *status.error.lock() = Some(msg);
        Self(status)
    }
}

impl Session for Unsupported {
    fn status(&self) -> &Arc<StreamStatus> {
        &self.0
    }
    fn stop(&mut self) {}
}

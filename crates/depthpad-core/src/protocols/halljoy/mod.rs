//! Analog protocols ported from HallJoy (https://github.com/PashOK7/HallJoy, AGPL-3.0, by PashOK7):
//! one module per protocol family, each a pure state machine for `platform::drivers::analog`.
//! Nobody has tried these with Depthpad yet: the keyboards using them are marked untested.
//!
//! HallJoy's reports include the report ID at index 0; here the ID is passed separately, so its
//! byte offsets are one less in these modules.

use std::collections::BTreeMap;

use super::analog::{AnalogProtocol, Keys};

pub mod apex;
pub mod aula_w669;
pub mod hex80;
mod hex80_tables;
pub mod ipi;
mod ipi_tables;
pub mod rongyuan;
pub mod sayo;
pub mod sparklink;
pub mod sparkplay;
mod jingtai_tables;
mod rongyuan_tables;

/// The protocol for a HallJoy-derived driver name.
pub fn for_driver(driver: &str, usb: &super::UsbInfo) -> Option<Box<dyn AnalogProtocol>> {
    let (vendor_id, product_id) = (usb.vendor_id, usb.product_id);
    Some(match driver {
        "aula-w669" => Box::new(aula_w669::AulaW669::default()),
        "rongyuan" => Box::new(rongyuan::RongYuan::new(vendor_id, product_id)),
        "sparkplay" => Box::new(sparkplay::SparkPlay::new(usb)),
        "hex80" => Box::new(hex80::Hex80::new(product_id)),
        "ipi" => Box::new(ipi::Ipi::default()),
        "sayo" => Box::new(sayo::Sayo::default()),
        "steelseries-apex" => Box::new(apex::Apex::default()),
        "sparklink" => Box::new(sparklink::SparkLink::default()),
        _ => return None,
    })
}

/// Key depth by matrix position, published by analog code. Two positions can carry the same
/// code (a remapped key): the deeper one wins, as in HallJoy's physical-key table.
#[derive(Default)]
pub(crate) struct Matrix {
    depth: BTreeMap<usize, (u16, f64)>,
}

impl Matrix {
    pub fn set(&mut self, position: usize, code: u16, depth: f64) {
        if code == 0 {
            return;
        }
        if depth > 0.0 {
            self.depth.insert(position, (code, depth.min(1.0)));
        } else {
            self.depth.remove(&position);
        }
    }
    pub fn clear(&mut self) {
        self.depth.clear();
    }
    pub fn keys(&self) -> Keys {
        let mut keys = Keys::new();
        for &(code, d) in self.depth.values() {
            let e = keys.entry(code).or_insert(0.0);
            *e = e.max(d);
        }
        keys
    }
}

/// A deadline for the current step of a protocol's setup, armed on the first tick after the
/// step starts (protocols don't see the clock until `tick`).
#[derive(Default)]
pub(crate) struct Deadline {
    at: Option<f64>,
    armed: bool,
}

impl Deadline {
    /// Start timing a step that may take `secs`.
    pub fn start(&mut self) {
        self.at = None;
        self.armed = true;
    }
    pub fn stop(&mut self) {
        self.armed = false;
    }
    /// Whether the step ran out of time.
    pub fn expired(&mut self, now: f64, secs: f64) -> bool {
        if !self.armed {
            return false;
        }
        let at = *self.at.get_or_insert(now + secs);
        now >= at
    }
}

/// Little-endian u16 at `i`.
pub(crate) fn le16(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d.get(i).copied().unwrap_or(0), d.get(i + 1).copied().unwrap_or(0)])
}

/// Big-endian u16 at `i`.
pub(crate) fn be16(d: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([d.get(i).copied().unwrap_or(0), d.get(i + 1).copied().unwrap_or(0)])
}

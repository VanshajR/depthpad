//! Analog protocols of other Hall-effect keyboards, ported from AnalogSense's JavaScript SDK
//! (https://github.com/AnalogSense/JavaScript-SDK, MIT licence, Copyright (c) 2024 Calamity, Inc.).
//! Nobody has tried these with Depthpad yet: the models using them are marked untested.
//!
//! Each protocol is pure: it says what to send to the keyboard and turns its input reports into
//! the set of pressed keys (HID usage, depth 0..1). The driver in `platform::drivers::analog`
//! does the I/O. Reports are given without their report ID, which is passed separately.

use std::collections::BTreeMap;

use super::analogsense_tables as t;

/// A report to send: report ID and data (without the ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    pub report_id: u8,
    pub data: Vec<u8>,
}

/// Pressed keys: HID usage -> depth 0..1.
pub type Keys = BTreeMap<u16, f64>;

pub trait AnalogProtocol: Send {
    /// Reports to send right after the keyboard is opened.
    fn start(&mut self) -> Vec<Out> {
        Vec::new()
    }
    /// An input report arrived. Returns the complete set of pressed keys when the report
    /// completes one, and what to send next.
    fn input(&mut self, report_id: u8, data: &[u8]) -> (Option<Keys>, Vec<Out>);
    /// Called about every 10 ms; for protocols that poll or keep the keyboard awake.
    fn tick(&mut self, _now: f64) -> Vec<Out> {
        Vec::new()
    }
}

fn out(report_id: u8, len: usize, head: &[u8]) -> Out {
    let mut data = vec![0u8; len];
    data[..head.len()].copy_from_slice(head);
    Out { report_id, data }
}

fn be16(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(i)?, *d.get(i + 1)?]))
}

// ------------------------------------------------------------------------------------- Wooting

/// Wooting analog interface v1 (usage page FF54): big-endian u16 usage, u8 value, repeated.
pub struct WootingV1;

impl AnalogProtocol for WootingV1 {
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let mut keys = Keys::new();
        let mut i = 0;
        while let (Some(code), Some(&v)) = (be16(d, i), d.get(i + 2)) {
            if code == 0 {
                break;
            }
            keys.insert(code, v as f64 / 255.0);
            i += 3;
        }
        (Some(keys), Vec::new())
    }
}

/// Wooting analog interface v2 (usage page FF53, firmware 2.13+): 4-byte entries of position,
/// key code, namespace and the low value bits, then the high value bits; 10-bit values.
pub struct WootingV2;

impl AnalogProtocol for WootingV2 {
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let mut keys = Keys::new();
        for e in d.chunks_exact(4) {
            let (code, packed, hi) = (e[1] as u16, e[2], e[3] as u16);
            let namespace = ((packed >> 2) & 0xF) as u16;
            let usage = (namespace << 8) | code;
            let value = (hi << 2) | ((packed >> 6) & 0x3) as u16;
            if usage == 0 {
                break;
            }
            if value != 0 && namespace == 0 {
                keys.insert(usage, value as f64 / 1023.0);
            }
        }
        (Some(keys), Vec::new())
    }
}

// --------------------------------------------------------------------------------------- Razer

/// Razer Huntsman analog reports (report 7 on the V2 Analog and Mini Analog; report 11 with a
/// third byte per key on the V3 Pro family). Razer Synapse must be running for them to be sent.
pub struct Razer {
    pub report_id: u8,
}

impl AnalogProtocol for Razer {
    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if id != self.report_id {
            return (None, Vec::new());
        }
        let step = if self.report_id == 11 { 3 } else { 2 };
        let mut keys = Keys::new();
        for e in d.chunks(step) {
            let (Some(&code), Some(&v)) = (e.first(), e.get(1)) else { break };
            if code == 0 {
                break;
            }
            if let Some(u) = t::lookup(t::RAZER, code) {
                keys.insert(u, v as f64 / 255.0);
            }
        }
        (Some(keys), Vec::new())
    }
}

// --------------------------------------------------------------------------------------- NuPhy

/// NuPhy: one key change per report (0xA0, big-endian scancode at 2, value 0..200 at 7).
#[derive(Default)]
pub struct Nuphy {
    keys: Keys,
}

impl AnalogProtocol for Nuphy {
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if d.first() != Some(&0xA0) {
            return (None, Vec::new());
        }
        let (Some(code), Some(&v)) = (be16(d, 2), d.get(7)) else { return (None, Vec::new()) };
        let usage = t::lookup(t::NUPHY, code).or((code > 0 && code < 0x100).then_some(code));
        let Some(usage) = usage else { return (None, Vec::new()) };
        if v == 0 {
            self.keys.remove(&usage);
        } else {
            self.keys.insert(usage, (v as f64 / 200.0).min(1.0));
        }
        (Some(self.keys.clone()), Vec::new())
    }
}

// ----------------------------------------------------------------------------------- DrunkDeer

/// DrunkDeer: poll with report 4 (b6 03 01); the answer comes in three parts (n = 0, 1, 2) of 59
/// key values each, 0..40.
#[derive(Default)]
pub struct DrunkDeer {
    keys: Keys,
    last_poll: f64,
}

impl DrunkDeer {
    fn poll() -> Out {
        out(0x04, 63, &[0xB6, 0x03, 0x01])
    }
}

impl AnalogProtocol for DrunkDeer {
    fn start(&mut self) -> Vec<Out> {
        vec![Self::poll()]
    }
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let Some(&n) = d.get(3) else { return (None, Vec::new()) };
        if n == 0 {
            self.keys.clear();
        }
        for (i, &v) in d.iter().enumerate().skip(4) {
            if v != 0 {
                if let Some(u) = t::lookup(t::DRUNKDEER, n as u16 * 59 + (i as u16 - 4)) {
                    self.keys.insert(u, (v as f64 / 40.0).min(1.0));
                }
            }
        }
        if n == 2 { (Some(self.keys.clone()), vec![Self::poll()]) } else { (None, Vec::new()) }
    }
    fn tick(&mut self, now: f64) -> Vec<Out> {
        // keep polling even if an answer went missing
        if now - self.last_poll > 0.05 {
            self.last_poll = now;
            vec![Self::poll()]
        } else {
            Vec::new()
        }
    }
}

// --------------------------------------------------------------------------- Keychron, Lemokey

/// Keychron and Lemokey HE boards: with the custom firmware's full analog report (byte 31 of
/// the hello reply = 0x45) all keys come in four parts of 30; otherwise keys are polled one by
/// one by matrix row and column (slow on official firmware).
pub struct Keychron {
    rows: usize,
    cols: usize,
    layout: &'static [u16],
    state: KcState,
    index: usize,
    keys: Keys,
}

enum KcState {
    Hello,
    Full,
    Single { travel_at: usize },
}

impl Keychron {
    pub fn new(matrix: (usize, usize, &'static [u16])) -> Self {
        Self { rows: matrix.0, cols: matrix.1, layout: matrix.2, state: KcState::Hello, index: 0, keys: Keys::new() }
    }
    fn all() -> Out {
        out(0, 32, &[0xA9, 0x31])
    }
    /// The request for the next key that exists, from `index` on.
    fn single(&mut self) -> Out {
        let size = self.rows * self.cols;
        for _ in 0..size {
            if self.layout[self.index] != 0 {
                break;
            }
            self.index = (self.index + 1) % size;
        }
        out(0, 32, &[0xA9, 0x30, (self.index / self.cols) as u8, (self.index % self.cols) as u8])
    }
    fn set(&mut self, usage: u16, travel: u8) {
        if usage == 0 || usage == 0x409 {
            return;
        }
        if travel >= 5 {
            self.keys.insert(usage, (travel as f64 / 235.0).min(1.0));
        } else {
            self.keys.remove(&usage);
        }
    }
}

impl AnalogProtocol for Keychron {
    fn start(&mut self) -> Vec<Out> {
        vec![out(0, 32, &[0xA9, 0x01])]
    }
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let size = self.rows * self.cols;
        match self.state {
            KcState::Hello => {
                let version = d.get(2).copied().unwrap_or(0);
                self.index = 0;
                if d.get(31) == Some(&0x45) {
                    self.state = KcState::Full;
                    (None, vec![Self::all()])
                } else {
                    self.state = KcState::Single { travel_at: if version >= 4 { 6 } else { 3 } };
                    (None, vec![self.single()])
                }
            }
            KcState::Full => {
                for i in 0..30 {
                    let li = self.index * 30 + i;
                    if li < size {
                        self.set(self.layout[li], d.get(2 + i).copied().unwrap_or(0));
                    }
                }
                self.index += 1;
                if self.index == 4 {
                    self.index = 0;
                    (Some(self.keys.clone()), vec![Self::all()])
                } else {
                    (None, Vec::new())
                }
            }
            KcState::Single { travel_at } => {
                self.set(self.layout[self.index], d.get(travel_at).copied().unwrap_or(0));
                self.index = (self.index + 1) % size;
                (Some(self.keys.clone()), vec![self.single()])
            }
        }
    }
}

// ------------------------------------------------------------------------------------ MadLions

/// MadLions: polled four keys at a time from an offset; big-endian travel at 7 + i*5 + 3, 0..350.
pub struct MadLions {
    layout: &'static [u16],
    offset: usize,
    keys: Keys,
}

impl MadLions {
    pub fn new(sixty: bool) -> Self {
        Self { layout: if sixty { t::MADLIONS_60 } else { t::MADLIONS_68 }, offset: 0, keys: Keys::new() }
    }
    fn request(&self) -> Out {
        out(0, 32, &[0x02, 0x96, 0x1C, 0x00, 0x00, 0x00, self.offset as u8, 0x04])
    }
}

impl AnalogProtocol for MadLions {
    fn start(&mut self) -> Vec<Out> {
        vec![self.request()]
    }
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        for i in 0..4 {
            let Some(&usage) = self.layout.get(self.offset + i) else { continue };
            if usage == 0 || usage == 0x409 {
                continue;
            }
            match be16(d, 7 + i * 5 + 3) {
                Some(0) | None => {
                    self.keys.remove(&usage);
                }
                Some(travel) => {
                    self.keys.insert(usage, (travel as f64 / 350.0).min(1.0));
                }
            }
        }
        self.offset += 4;
        if self.offset >= self.layout.len() {
            self.offset = 0;
        }
        (Some(self.keys.clone()), vec![self.request()])
    }
}

// ------------------------------------------------------------------------- Bytech (Redragon)

/// Bytech BY3265H boards (Redragon K709 HE, possibly RK68 HE, DAREU COOL68 8K): report 9 with
/// command 0x97 and a checksum; each answer lists pressed keys with their distance, 0..355.
#[derive(Default)]
pub struct Bytech {
    last_poll: f64,
}

impl Bytech {
    pub fn poll() -> Out {
        let mut d = vec![0u8; 63];
        d[0] = 0x97;
        let sum: u32 = 9 + d[..62].iter().map(|&b| b as u32).sum::<u32>();
        d[62] = 255 - (sum % 256) as u8;
        Out { report_id: 9, data: d }
    }
}

impl AnalogProtocol for Bytech {
    fn start(&mut self) -> Vec<Out> {
        vec![Self::poll()]
    }
    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if d.first() != Some(&0x97) || d.get(1) != Some(&0x01) {
            return (None, Vec::new());
        }
        let mut keys = Keys::new();
        let count = d.get(5).copied().unwrap_or(0) as usize;
        for i in (0..count).step_by(4) {
            let (Some(pos), Some(dist)) = (be16(d, 6 + i), be16(d, 8 + i)) else { break };
            if let Some(u) = t::lookup(t::BYTECH, pos) {
                if dist > 10 {
                    keys.insert(u, (dist as f64 / 355.0).min(1.0));
                }
            }
        }
        (Some(keys), vec![Self::poll()])
    }
    fn tick(&mut self, now: f64) -> Vec<Out> {
        if now - self.last_poll > 1.0 {
            self.last_poll = now;
            vec![Self::poll()]
        } else {
            Vec::new()
        }
    }
}

/// The protocol a driver name and the opened collection call for.
pub fn for_driver(driver: &str, product_id: u16, usage_page: u16) -> Option<Box<dyn AnalogProtocol>> {
    Some(match driver {
        "wooting" => {
            if usage_page == 0xFF54 { Box::new(WootingV1) } else { Box::new(WootingV2) }
        }
        "razer" => Box::new(Razer { report_id: if matches!(product_id, 0x02A6 | 0x02A7 | 0x02B0) { 11 } else { 7 } }),
        "nuphy" => Box::new(Nuphy::default()),
        "drunkdeer" => Box::new(DrunkDeer::default()),
        "keychron" => Box::new(Keychron::new(match product_id {
            0x0B10..=0x0B12 => t::KEYCHRON_Q1_HE,
            0x0B30 => t::KEYCHRON_Q3_HE,
            0x0B50 => t::KEYCHRON_Q5_HE,
            0x0610 => t::LEMOKEY_P1_HE,
            _ => t::KEYCHRON_K2_HE,
        })),
        "madlions" => Box::new(MadLions::new(matches!(product_id, 0x1053 | 0x1055 | 0x1056 | 0x105D))),
        "bytech" => Box::new(Bytech::default()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wooting_v2_entries() {
        // W (0x1A) at 512/1023, then a terminator
        let d = [0, 0x1A, 0b00_0000_00, 128, 0, 0, 0, 0];
        let (k, _) = WootingV2.input(0, &d);
        let k = k.unwrap();
        assert!((k[&0x1A] - 512.0 / 1023.0).abs() < 1e-9);
        assert_eq!(k.len(), 1);
    }

    #[test]
    fn wooting_v1_entries() {
        let d = [0x00, 0x1A, 255, 0x00, 0x04, 51, 0, 0, 0];
        let k = WootingV1.input(0, &d).0.unwrap();
        assert_eq!((k[&0x1A], (k[&0x04] * 100.0).round()), (1.0, 20.0));
    }

    #[test]
    fn razer_reports() {
        let k = Razer { report_id: 7 }.input(7, &[0x12, 255, 0x1F, 0, 0]).0.unwrap(); // W full, A at 0
        assert_eq!((k[&0x1A], k[&0x04]), (1.0, 0.0));
        assert!(Razer { report_id: 7 }.input(1, &[0x12, 255]).0.is_none(), "other reports ignored");
        let k = Razer { report_id: 11 }.input(11, &[0x12, 128, 9, 0x1F, 255, 9, 0]).0.unwrap();
        assert_eq!(k.len(), 2);
    }

    #[test]
    fn nuphy_tracks_changes() {
        let mut n = Nuphy::default();
        let k = n.input(0, &[0xA0, 0, 0x00, 0x1A, 0, 0, 0, 100]).0.unwrap();
        assert_eq!(k[&0x1A], 0.5);
        let k = n.input(0, &[0xA0, 0, 0x02, 0x00, 0, 0, 0, 200]).0.unwrap(); // Left Shift
        assert_eq!(k[&0xE1], 1.0);
        let k = n.input(0, &[0xA0, 0, 0x00, 0x1A, 0, 0, 0, 0]).0.unwrap();
        assert!(!k.contains_key(&0x1A));
    }

    #[test]
    fn drunkdeer_three_parts() {
        let mut p = DrunkDeer::default();
        let mut part = |n: u8, idx: usize, v: u8| {
            let mut d = vec![0u8; 63];
            d[3] = n;
            if v > 0 {
                d[4 + idx] = v;
            }
            p.input(0, &d)
        };
        assert!(part(0, 2 * 21 + 2 - 0, 20).0.is_none()); // W in part 0 (index 44)
        assert!(part(1, 0, 0).0.is_none());
        let (k, next) = part(2, 0, 0);
        assert_eq!(k.unwrap()[&0x1A], 0.5);
        assert_eq!(next[0].report_id, 4);
    }

    #[test]
    fn keychron_full_report_mode() {
        let mut p = Keychron::new(t::KEYCHRON_Q1_HE);
        assert_eq!(p.start()[0].data[..2], [0xA9, 0x01]);
        let mut hello = vec![0u8; 32];
        hello[31] = 0x45;
        let (_, next) = p.input(0, &hello);
        assert_eq!(next[0].data[..2], [0xA9, 0x31]);
        // W is row 2, column 2 -> index 32 -> part 1, slot 2
        let mut keys = None;
        for part in 0..4 {
            let mut d = vec![0u8; 32];
            if part == 1 {
                d[2 + 2] = 235;
            }
            keys = p.input(0, &d).0.or(keys);
        }
        assert_eq!(keys.unwrap()[&0x1A], 1.0);
    }

    #[test]
    fn keychron_single_key_polling() {
        let mut p = Keychron::new(t::KEYCHRON_K2_HE);
        p.start();
        let (_, next) = p.input(0, &[0u8; 32]); // no full report: poll key by key
        assert_eq!(next[0].data[..4], [0xA9, 0x30, 0, 0]);
    }

    #[test]
    fn madlions_four_at_a_time() {
        let mut p = MadLions::new(true);
        let mut d = vec![0u8; 32];
        // second key in the first group of four is "1" (0x1E): travel 175 of 350
        d[7 + 5 + 3] = 0;
        d[7 + 5 + 4] = 175;
        let (k, next) = p.input(0, &d);
        assert_eq!(k.unwrap()[&0x1E], 0.5);
        assert_eq!(next[0].data[6], 4, "next offset");
    }

    #[test]
    fn bytech_checksum_and_keys() {
        let poll = Bytech::poll();
        let sum: u32 = 9 + poll.data.iter().map(|&b| b as u32).sum::<u32>();
        assert_eq!(sum % 256, 255, "checksum makes the total 255 mod 256");
        let mut d = vec![0u8; 63];
        d[0] = 0x97;
        d[1] = 0x01;
        d[5] = 4;
        d[6..10].copy_from_slice(&[0, 30, 0, 200]); // W (bytech 30), distance 200
        let k = Bytech::default().input(9, &d).0.unwrap();
        assert!((k[&0x1A] - 200.0 / 355.0).abs() < 1e-9);
    }
}

//! ATK (Hex80, EDGE, RS6/RS7, 68 V2/V3/RX) and MADLIONS MAD68 HE V2 keyboards, from HallJoy's
//! `hex80_protocol.cpp` and `hex80_backend.cpp`.
//!
//! QMK raw HID collection (FF60:0061), 32-byte reports. 02 96 24 reads the travel scale (some
//! models have a fixed one), 03 96 19 takes a Hex80 out of calibration mode, and 02 96 1C reads
//! the travel of four matrix slots at a time (offset big-endian, then 5-byte records: ADC,
//! travel, status), polled round the whole matrix.

use super::{be16, Matrix};
use crate::protocols::analog::{AnalogProtocol, Keys, Out};
use crate::protocols::halljoy::hex80_tables::MODELS;

pub struct Model {
    /// The name the layout catalogue knows this model by.
    pub product: &'static str,
    pub pids: &'static [u16],
    /// 0: read the scale from the keyboard.
    pub fixed_max: u16,
    /// Travel at or under this counts as released.
    pub deadzone: u16,
    /// Send 03 96 19 after connecting (Hex80 only).
    pub calibration_finish: bool,
    /// Analog code of each matrix slot.
    pub slots: &'static [u16],
}

pub fn find(pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.pids.contains(&pid))
}

const GET: u8 = 0x02;
const SET: u8 = 0x03;
const CUSTOM: u8 = 0x96;
const TRAVEL_INFO: u8 = 0x24;
const TRAVEL_BUFFER: u8 = 0x1C;
const CALIBRATION_FINISH: u8 = 0x19;
const CHUNK: u16 = 4;
const LEN: usize = 32;
/// A reply this late is lost: ask again.
const REPLY_SECS: f64 = 0.1;
/// Failed requests in a row before starting over.
const FAILURES: u32 = 10;
const ATTEMPTS: u32 = 3;

fn request(op: u8, sub: u8, offset: u16, size: u8) -> Out {
    let mut o = Out::write(0, LEN, &[op, CUSTOM, sub]);
    if sub == TRAVEL_BUFFER {
        o.data[5] = (offset >> 8) as u8;
        o.data[6] = offset as u8;
        o.data[7] = size;
    }
    o
}

/// The reply's payload (with or without a leading report-ID zero), if it answers op/sub.
fn payload(d: &[u8], op: u8, sub: u8) -> Option<&[u8]> {
    if d.len() >= 3 && d[0] == op && d[1] == CUSTOM && d[2] == sub {
        Some(d)
    } else if d.len() >= 4 && d[0] == 0 && d[1] == op && d[2] == CUSTOM && d[3] == sub {
        Some(&d[1..])
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Scale,
    Poll(u16),
    Failed,
}

pub struct Hex80 {
    model: Option<&'static Model>,
    pid: u16,
    step: Step,
    max: u16,
    sent_at: Option<f64>,
    failures: u32,
    attempts: u32,
    matrix: Matrix,
}

impl Hex80 {
    pub fn new(pid: u16) -> Self {
        Self { model: find(pid), pid, step: Step::Scale, max: 0, sent_at: None, failures: 0, attempts: 0, matrix: Matrix::default() }
    }

    fn ask(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.sent_at = None;
        match step {
            Step::Scale => vec![request(GET, TRAVEL_INFO, 0, 0)],
            Step::Poll(offset) => {
                let slots = self.model.map_or(0, |m| m.slots.len() as u16);
                vec![request(GET, TRAVEL_BUFFER, offset, CHUNK.min(slots - offset) as u8)]
            }
            Step::Failed => Vec::new(),
        }
    }

    /// Scale known: (once) leave calibration mode, then poll from the first slot.
    fn begin_polling(&mut self) -> Vec<Out> {
        let mut outs = Vec::new();
        if self.model.is_some_and(|m| m.calibration_finish) {
            outs.push(request(SET, CALIBRATION_FINISH, 0, 0));
        }
        outs.extend(self.ask(Step::Poll(0)));
        outs
    }
}

impl AnalogProtocol for Hex80 {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.failures = 0;
        self.attempts += 1;
        let Some(model) = self.model else {
            self.step = Step::Failed;
            return Vec::new();
        };
        if self.attempts > ATTEMPTS {
            self.step = Step::Failed;
            return Vec::new();
        }
        if model.fixed_max != 0 {
            self.max = model.fixed_max;
            return self.begin_polling();
        }
        self.ask(Step::Scale)
    }

    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let Some(model) = self.model else { return (None, Vec::new()) };
        match self.step {
            Step::Scale => {
                if let Some(p) = payload(d, GET, TRAVEL_INFO).filter(|p| p.len() >= 5) {
                    let max = be16(p, 3);
                    if (256..=20000).contains(&max) {
                        self.max = max;
                        return (None, self.begin_polling());
                    }
                }
            }
            Step::Poll(offset) => {
                let size = CHUNK.min(model.slots.len() as u16 - offset) as usize;
                let Some(p) = payload(d, GET, TRAVEL_BUFFER) else { return (None, Vec::new()) };
                if p.len() < 8 + size * 5 || be16(p, 5) != offset || p[7] as usize != size {
                    return (None, Vec::new()); // a late reply to an earlier request
                }
                let limit = (self.max.max(3300) as u32 * 2).min(0xFFFF);
                for i in 0..size {
                    let travel = be16(p, 8 + i * 5 + 2);
                    if travel as u32 > limit {
                        return (None, Vec::new());
                    }
                    let dz = model.deadzone;
                    let depth = if travel <= dz || self.max <= dz { 0.0 } else { (travel - dz) as f64 / (self.max - dz) as f64 };
                    let slot = offset as usize + i;
                    self.matrix.set(slot, model.slots[slot], depth);
                }
                self.failures = 0;
                self.attempts = 0;
                let next = offset + CHUNK;
                if next as usize >= model.slots.len() {
                    return (Some(self.matrix.keys()), self.ask(Step::Poll(0)));
                }
                return (None, self.ask(Step::Poll(next)));
            }
            Step::Failed => {}
        }
        (None, Vec::new())
    }

    fn tick(&mut self, now: f64) -> Vec<Out> {
        if self.step == Step::Failed {
            return Vec::new();
        }
        let sent = *self.sent_at.get_or_insert(now);
        if now - sent < REPLY_SECS {
            return Vec::new();
        }
        self.failures += 1;
        if self.failures >= FAILURES {
            return self.start();
        }
        self.ask(self.step) // ask again
    }

    fn identity(&self) -> Option<String> {
        self.model.map(|m| m.product.to_string())
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| match self.model {
            None => format!("product {:04X} isn't one Depthpad knows the key matrix of yet", self.pid),
            Some(_) => "the keyboard didn't answer analog requests".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(offset: u16, travels: &[u16]) -> Vec<u8> {
        let mut d = vec![0u8; 32];
        (d[0], d[1], d[2]) = (GET, CUSTOM, TRAVEL_BUFFER);
        (d[5], d[6], d[7]) = ((offset >> 8) as u8, offset as u8, travels.len() as u8);
        for (i, t) in travels.iter().enumerate() {
            d[8 + i * 5 + 2] = (t >> 8) as u8;
            d[8 + i * 5 + 3] = *t as u8;
        }
        d
    }

    #[test]
    fn hex80_session() {
        let mut p = Hex80::new(0x1176);
        assert_eq!(p.identity().as_deref(), Some("HEX80-ANSI"));
        let o = p.start();
        assert_eq!(o[0].data[..3], [GET, CUSTOM, TRAVEL_INFO]);
        // scale 330 (0.01 mm), with a report-ID zero in front this time
        let (_, o) = p.input(0, &[0, GET, CUSTOM, TRAVEL_INFO, 0x01, 0x4A]);
        assert_eq!(p.max, 330);
        assert_eq!(o[0].data[..3], [SET, CUSTOM, CALIBRATION_FINISH]);
        assert_eq!((o[1].data[2], o[1].data[6], o[1].data[7]), (TRAVEL_BUFFER, 0, 4));
        // W is slot 36 (row 2, column 2): chunk 36..39; walk there
        let m = find(0x1176).unwrap();
        let w = m.slots.iter().position(|&c| c == 0x1A).unwrap() as u16;
        assert_eq!(w, 36);
        let mut keys = None;
        let mut offset = 0;
        while (offset as usize) < m.slots.len() {
            let n = 4.min(m.slots.len() - offset as usize);
            let mut t = vec![0u16; n];
            if offset == w {
                t[0] = 8 + (330 - 8) / 2; // half way past the deadzone
            }
            let (k, o) = p.input(0, &chunk(offset, &t));
            keys = k.or(keys);
            offset += 4;
            assert_eq!(o.len(), 1);
        }
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
    }

    #[test]
    fn fixed_scale_models_skip_the_scale_request() {
        // MAD68 HE V2: fixed 3.3 mm, no calibration command
        let mut p = Hex80::new(0x1125);
        let o = p.start();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].data[2], TRAVEL_BUFFER);
    }

    #[test]
    fn late_replies_are_ignored_and_silence_retries() {
        let mut p = Hex80::new(0x1125);
        p.start();
        assert!(p.input(0, &chunk(8, &[0, 0, 0, 0])).1.is_empty());
        p.tick(0.0);
        assert_eq!(p.tick(0.2)[0].data[6], 0); // asked again for slot 0
    }

    #[test]
    fn unknown_product() {
        let mut p = Hex80::new(0x0001);
        p.start();
        assert!(p.problem().unwrap().contains("0001"));
    }
}

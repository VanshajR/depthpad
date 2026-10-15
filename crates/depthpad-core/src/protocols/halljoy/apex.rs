//! SteelSeries Apex Pro (and Apex Pro TKL / Gen 3 on the same firmware), from HallJoy's
//! `steelseries_apex_protocol.h` and `steelseries_apex_backend.cpp`. Read-only: only the
//! firmware version, calibration and depth requests HallJoy checked are ever sent.
//!
//! Vendor collection FFC0:0001, 64-byte reports. 0x90 gives the firmware version (only 4.16.8
//! is known); 0xDA reads each sensor's calibrated range (up to 12 sensors a request, refreshed
//! every two seconds); 0xD7 <bank> reads 14 sensors' raw values, banks 1-5.

use super::le16;
use crate::protocols::analog::{AnalogProtocol, Keys, Out};

const LEN: usize = 64;
const SENSORS: usize = 68;
/// HID usage of each sensor (the app's inverse ROM map; 240 is not a keyboard key).
const MAP: [u16; 70] = [
    53, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46, 137, 43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48, 49, 57, 4, 22,
    7, 9, 10, 11, 13, 14, 15, 51, 52, 50, 40, 225, 100, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 135, 229, 224, 227, 226, 139,
    44, 138, 136, 230, 231, 240, 228, 42, 0, 0,
];
const REPLY_SECS: f64 = 0.1;
const RANGE_REFRESH_SECS: f64 = 2.0;
const ATTEMPTS: u32 = 3;

fn version_request() -> Out {
    Out::write(0, LEN, &[0x90])
}

fn range_request(start: usize, count: usize) -> Out {
    let mut o = Out::write(0, LEN, &[0xDA, count as u8]);
    for i in 0..count {
        o.data[2 + i * 5] = MAP[start + i] as u8;
    }
    o
}

fn depth_request(bank: u8) -> Out {
    Out::write(0, LEN, &[0xD7, bank])
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Version,
    Ranges(usize),
    Depth(u8),
    Failed,
}

pub struct Apex {
    step: Step,
    sent_at: Option<f64>,
    ranges_at: f64,
    now: f64,
    attempts: u32,
    /// Calibrated (low, high) of each sensor; (0, 0) for one without.
    ranges: [(u16, u16); SENSORS],
    next: [(u16, u16); SENSORS],
    raw: [u16; 70],
    reason: Option<&'static str>,
}

impl Default for Apex {
    fn default() -> Self {
        Self {
            step: Step::Version, sent_at: None, ranges_at: 0.0, now: 0.0, attempts: 0, ranges: [(0, 0); SENSORS],
            next: [(0, 0); SENSORS], raw: [0; 70], reason: None,
        }
    }
}

impl Apex {
    fn ask(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.sent_at = None;
        match step {
            Step::Version => vec![version_request()],
            Step::Ranges(start) => vec![range_request(start, 12.min(SENSORS - start))],
            Step::Depth(bank) => vec![depth_request(bank)],
            Step::Failed => Vec::new(),
        }
    }

    fn fail(&mut self, why: &'static str) -> Vec<Out> {
        self.reason = Some(why);
        self.step = Step::Failed;
        Vec::new()
    }

    fn keys(&self) -> Keys {
        let mut keys = Keys::new();
        for i in 0..SENSORS {
            let (lo, hi) = self.ranges[i];
            let (raw, code) = (self.raw[i], MAP[i]);
            if hi > lo && code != 240 && code != 0 && raw > lo {
                keys.insert(code, if raw >= hi { 1.0 } else { (raw - lo) as f64 / (hi - lo) as f64 });
            }
        }
        keys
    }
}

impl AnalogProtocol for Apex {
    fn start(&mut self) -> Vec<Out> {
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            return self.fail("the keyboard didn't answer analog requests");
        }
        self.ask(Step::Version)
    }

    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if id != 0 || d.len() < LEN {
            return (None, Vec::new());
        }
        match self.step {
            Step::Version => {
                if d.starts_with(b"4.16.8\0") {
                    self.next = [(0, 0); SENSORS];
                    return (None, self.ask(Step::Ranges(0)));
                }
                return (None, self.fail("its firmware isn't 4.16.8, the version Depthpad was written for"));
            }
            Step::Ranges(start) => {
                let count = 12.min(SENSORS - start);
                for i in 0..count {
                    let q = &d[i * 5..];
                    if q[0] as u16 != MAP[start + i] {
                        return (None, Vec::new()); // not this reply
                    }
                    let (hi, lo) = (le16(q, 1), le16(q, 3));
                    if hi > 4095 || lo > 4095 {
                        return (None, Vec::new());
                    }
                    // positions a region doesn't have keep reversed factory values: no range
                    self.next[start + i] = if hi > lo + 16 { (lo, hi) } else { (0, 0) };
                }
                if start + count < SENSORS {
                    return (None, self.ask(Step::Ranges(start + count)));
                }
                // the main block must be calibrated
                if (14..40).any(|i| self.next[i].1 <= self.next[i].0) {
                    return (None, self.fail("the keyboard has no calibration for its main keys"));
                }
                self.ranges = self.next;
                self.ranges_at = self.now;
                self.attempts = 0;
                return (None, self.ask(Step::Depth(1)));
            }
            Step::Depth(bank) => {
                if d[56..].iter().any(|&b| b != 0) || (0..28).any(|i| le16(d, 2 * i) > 4095) {
                    return (None, Vec::new());
                }
                for i in 0..14 {
                    self.raw[(bank as usize - 1) * 14 + i] = le16(d, 28 + 2 * i);
                }
                if bank < 5 {
                    return (None, self.ask(Step::Depth(bank + 1)));
                }
                let keys = self.keys();
                let outs = if self.now - self.ranges_at >= RANGE_REFRESH_SECS {
                    self.next = [(0, 0); SENSORS];
                    self.ask(Step::Ranges(0))
                } else {
                    self.ask(Step::Depth(1))
                };
                return (Some(keys), outs);
            }
            Step::Failed => {}
        }
        (None, Vec::new())
    }

    fn tick(&mut self, now: f64) -> Vec<Out> {
        self.now = now;
        if self.step == Step::Failed {
            return Vec::new();
        }
        let sent = *self.sent_at.get_or_insert(now);
        if now - sent > REPLY_SECS { self.start() } else { Vec::new() }
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| self.reason.unwrap_or_default().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apex_session() {
        let mut p = Apex::default();
        assert_eq!(p.start()[0].data[0], 0x90);
        let mut v = vec![0u8; LEN];
        v[..7].copy_from_slice(b"4.16.8\0");
        let (_, o) = p.input(0, &v);
        assert_eq!(o[0].data[..3], [0xDA, 12, 53]);
        // every sensor: low 1000, high 3000
        let mut start = 0;
        while let Step::Ranges(s) = p.step {
            assert_eq!(s, start);
            let n = 12.min(SENSORS - s);
            let mut r = vec![0u8; LEN];
            for i in 0..n {
                r[i * 5] = MAP[s + i] as u8;
                r[i * 5 + 1..i * 5 + 3].copy_from_slice(&3000u16.to_le_bytes());
                r[i * 5 + 3..i * 5 + 5].copy_from_slice(&1000u16.to_le_bytes());
            }
            p.input(0, &r);
            start += n;
        }
        assert_eq!(p.step, Step::Depth(1));
        // W is sensor 16: bank 2 (sensors 14-27), its third value
        let w = MAP.iter().position(|&h| h == 0x1A).unwrap();
        assert_eq!(w, 16);
        let mut keys = None;
        for bank in 1..=5u8 {
            let mut r = vec![0u8; LEN];
            if bank == 2 {
                r[28 + 2 * 2..28 + 2 * 2 + 2].copy_from_slice(&2000u16.to_le_bytes());
            }
            keys = p.input(0, &r).0.or(keys);
        }
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
    }

    #[test]
    fn other_firmware_is_left_alone() {
        let mut p = Apex::default();
        p.start();
        let mut v = vec![0u8; LEN];
        v[..7].copy_from_slice(b"4.17.0\0");
        assert!(p.input(0, &v).1.is_empty());
        assert!(p.problem().unwrap().contains("4.16.8"));
    }
}

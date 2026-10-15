//! SayoDevice O3C (three analog keys), from HallJoy's `sayo_o3c_protocol.h` and `backend_sayo.inc`.
//!
//! Collection FF12:0002, 1024-byte output reports with ID 0x22. Frames: echo, checksum (u16 sum of
//! the frame's words, its own word left out), length, command, index, payload. Every 8 ms the
//! fixed request 12/15/01 asks for the three keys' raw depth (0-4000 is full travel).

use crate::protocols::analog::{AnalogProtocol, Keys, Out};

const ID: u8 = 0x22;
/// Factory keys: Z, X, C.
pub const FACTORY: [u16; 3] = [0x1D, 0x1B, 0x06];
const POLL_SECS: f64 = 0.008;
const FULL: u32 = 4000;

fn u16le(p: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([p[i], p[i + 1]])
}

/// The fixed depth request (HallJoy's kPollPrefix, report ID first in HallJoy).
pub fn poll() -> Out {
    Out::write(ID, 1023, &[0x12, 0x3C, 0x13, 0x05, 0x00, 0x15, 0x01])
}

/// The three raw depths from a reply (without the report ID), if it is one.
pub fn depth_reply(d: &[u8]) -> Option<[u16; 3]> {
    // frame as HallJoy sees it: 22 echo sum(2) length(2) command index payload
    let mut f = vec![ID];
    f.extend_from_slice(d);
    if f.len() < 8 {
        return None;
    }
    let length = u16le(&f, 4) as usize;
    let end = (length + 5) & !1;
    if !(4..=1020).contains(&length) || length + 4 > f.len() || end > f.len() {
        return None;
    }
    let sum = (0..end).step_by(2).filter(|&i| i != 2).fold(0u16, |s, i| s.wrapping_add(u16le(&f, i)));
    if sum != u16le(&f, 2) || f[1] != 0x12 || f[6] != 0x15 || f[7] != 1 || length - 4 != 6 {
        return None;
    }
    let raw = [u16le(&f, 8), u16le(&f, 10), u16le(&f, 12)];
    raw.iter().all(|&r| r <= 8000).then_some(raw)
}

#[derive(Default)]
pub struct Sayo {
    last_poll: f64,
    seen: bool,
}

impl AnalogProtocol for Sayo {
    fn start(&mut self) -> Vec<Out> {
        vec![poll()]
    }

    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if id != ID {
            return (None, Vec::new());
        }
        let Some(raw) = depth_reply(d) else { return (None, Vec::new()) };
        self.seen = true;
        let mut keys = Keys::new();
        for (i, &r) in raw.iter().enumerate() {
            let milli = ((r as u32 * 1000 + FULL / 2) / FULL).min(1000);
            if milli >= 4 {
                keys.insert(FACTORY[i], milli as f64 / 1000.0);
            }
        }
        (Some(keys), Vec::new())
    }

    fn tick(&mut self, now: f64) -> Vec<Out> {
        if now - self.last_poll < POLL_SECS {
            return Vec::new();
        }
        self.last_poll = now;
        vec![poll()]
    }

    fn identity(&self) -> Option<String> {
        self.seen.then(|| "O3C-8089-0009".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(raw: [u16; 3]) -> Vec<u8> {
        let mut f = vec![ID, 0x12, 0, 0, 10, 0, 0x15, 1];
        for r in raw {
            f.extend_from_slice(&r.to_le_bytes());
        }
        f.resize(64, 0);
        let end = (10 + 5) & !1;
        let sum = (0..end).step_by(2).filter(|&i| i != 2).fold(0u16, |s, i| s.wrapping_add(u16le(&f, i)));
        f[2..4].copy_from_slice(&sum.to_le_bytes());
        f[1..].to_vec()
    }

    #[test]
    fn poll_matches_halljoy() {
        // 22 12 3C 13 05 00 15 01: the checksum 0x133C covers 0x1222 + 0x0005 + 0x0115
        let o = poll();
        assert_eq!((o.report_id, &o.data[..7]), (ID, &[0x12, 0x3C, 0x13, 0x05, 0x00, 0x15, 0x01][..]));
        assert_eq!(0x1222u16.wrapping_add(0x0005).wrapping_add(0x0115), 0x133C);
    }

    #[test]
    fn depth() {
        let mut p = Sayo::default();
        let keys = p.input(ID, &reply([2000, 0, 4000])).0.unwrap();
        assert_eq!(keys[&0x1D], 0.5);
        assert_eq!(keys[&0x06], 1.0);
        assert!(!keys.contains_key(&0x1B));
        let mut bad = reply([1, 2, 3]);
        bad[3] ^= 1;
        assert!(p.input(ID, &bad).0.is_none());
    }
}

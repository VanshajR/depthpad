//! IROK MG75 Max and Everglide SU75 Pro (JingTai V2 "SparkLink" firmware), from HallJoy's
//! `backend_sparklink.inc` and `sparklink_key_codes.h`.
//!
//! Vendor collection FFB0:0001, 64-byte packets. 01 02 is device info; 03 01 00 <row> gives a
//! row's 21 key codes (u16 LE from byte 4), and 04 03 01 <row> the row's 21 travels, polled one
//! row at a time. Travel is scaled by each key's largest reading so far (3.0-5.0 mm, starting at
//! 3.5), as HallJoy does.

use super::{le16, Matrix};
use crate::protocols::analog::{AnalogProtocol, Keys, Out};

const LEN: usize = 64;
const ROWS: u8 = 8;
const COLS: usize = 21;
/// HallJoy: 250 ms per transaction.
const REPLY_SECS: f64 = 0.25;
const FAILURES: u32 = 8;
const ATTEMPTS: u32 = 3;
const FULL_DEFAULT: u16 = 3500;

/// Key code -> analog code (0xF101 is Fn).
fn decode_key(code: u16) -> u16 {
    match code {
        0xF101 => 0x409,
        0..=0xFF => code,
        _ => 0,
    }
}

fn request(head: &[u8]) -> Out {
    Out::write(0, LEN, head)
}

/// The packet in a report that starts with `head` (the report ID may or may not be in front).
fn packet<'a>(d: &'a [u8], head: &[u8]) -> Option<&'a [u8]> {
    (0..=1).map(|off| d.get(off..)).flatten().find(|p| p.len() >= LEN && p.starts_with(head))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Info,
    Layout(u8),
    Poll(u8),
    Failed,
}

pub struct SparkLink {
    step: Step,
    sent_at: Option<f64>,
    failures: u32,
    attempts: u32,
    codes: [[u16; COLS]; ROWS as usize],
    rows: u8,
    full: Vec<u16>,
    matrix: Matrix,
    empty_run: u8,
}

impl Default for SparkLink {
    fn default() -> Self {
        Self {
            step: Step::Info, sent_at: None, failures: 0, attempts: 0, codes: [[0; COLS]; ROWS as usize], rows: 0,
            full: vec![FULL_DEFAULT; ROWS as usize * COLS], matrix: Matrix::default(), empty_run: 0,
        }
    }
}

impl SparkLink {
    fn ask(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.sent_at = None;
        match step {
            Step::Info => vec![request(&[0x01, 0x02])],
            Step::Layout(r) => vec![request(&[0x03, 0x01, 0x00, r])],
            Step::Poll(r) => vec![request(&[0x04, 0x03, 0x01, r])],
            Step::Failed => Vec::new(),
        }
    }

    /// The next row with keys after `r`, wrapping round.
    fn next_row(&self, r: u8) -> u8 {
        (1..=self.rows)
            .map(|i| (r + i) % self.rows)
            .find(|&n| self.codes[n as usize].iter().any(|&c| decode_key(c) != 0))
            .unwrap_or(0)
    }

    fn layout_done(&mut self) -> Vec<Out> {
        if self.rows == 0 {
            self.step = Step::Failed;
            return Vec::new();
        }
        self.attempts = 0;
        let first = self.next_row(self.rows - 1);
        self.ask(Step::Poll(first))
    }
}

impl AnalogProtocol for SparkLink {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.failures = 0;
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            self.step = Step::Failed;
            return Vec::new();
        }
        self.ask(Step::Info)
    }

    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        match self.step {
            Step::Info => {
                if packet(d, &[0x01, 0x02]).is_some_and(|p| p[2] == 0x01) {
                    (self.rows, self.empty_run) = (0, 0);
                    return (None, self.ask(Step::Layout(0)));
                }
            }
            Step::Layout(r) => {
                if let Some(p) = packet(d, &[0x03, 0x01, 0x00, r]) {
                    let row: [u16; COLS] = std::array::from_fn(|c| le16(p, 4 + c * 2));
                    self.codes[r as usize] = row;
                    if row.iter().any(|&c| c != 0) {
                        (self.rows, self.empty_run) = (r + 1, 0);
                    } else {
                        self.empty_run += 1;
                    }
                    // two empty rows after the last one with keys end the layout
                    if r + 1 >= ROWS || (self.rows > 0 && self.empty_run >= 2) {
                        return (None, self.layout_done());
                    }
                    return (None, self.ask(Step::Layout(r + 1)));
                }
            }
            Step::Poll(r) => {
                if let Some(p) = packet(d, &[0x04, 0x03, 0x01, r]) {
                    for c in 0..COLS {
                        let code = decode_key(self.codes[r as usize][c]);
                        let raw = le16(p, 4 + c * 2);
                        let i = r as usize * COLS + c;
                        let full = &mut self.full[i];
                        *full = (*full).clamp(3000, 5000);
                        if raw > *full && raw <= 5000 {
                            *full = raw;
                        }
                        self.matrix.set(i, code, raw as f64 / *full as f64);
                    }
                    self.failures = 0;
                    let next = self.next_row(r);
                    return (Some(self.matrix.keys()), self.ask(Step::Poll(next)));
                }
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
        match self.step {
            // a row that never answers ends the layout
            Step::Layout(_) if self.failures >= 3 => self.layout_done(),
            _ if self.failures >= FAILURES => self.start(),
            s => self.ask(s),
        }
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| "the keyboard didn't report its key layout".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(head: &[u8], values: &[(usize, u16)]) -> Vec<u8> {
        let mut d = vec![0u8; LEN];
        d[..head.len()].copy_from_slice(head);
        for &(c, v) in values {
            d[4 + c * 2..6 + c * 2].copy_from_slice(&v.to_le_bytes());
        }
        d
    }

    #[test]
    fn mg75_max_session() {
        let mut p = SparkLink::default();
        assert_eq!(p.start()[0].data[..2], [1, 2]);
        let (_, o) = p.input(0, &pkt(&[1, 2, 1], &[]));
        assert_eq!(o[0].data[..4], [3, 1, 0, 0]);
        // rows 0-4 have keys (W at row 2, column 2; Fn on row 4), rows 5 and 6 are empty
        for r in 0..7u8 {
            let keys: Vec<(usize, u16)> = match r {
                2 => vec![(2, 0x1A)],
                4 => vec![(11, 0xF101)],
                0..=4 => vec![(0, 0x29)],
                _ => vec![],
            };
            p.input(0, &pkt(&[3, 1, 0, r], &keys));
        }
        assert_eq!(p.rows, 5);
        assert_eq!(p.step, Step::Poll(0));
        // poll until row 2, W at 1750 of 3500
        let mut keys = None;
        for r in [0u8, 1, 2] {
            let v = if r == 2 { vec![(2, 1750)] } else { vec![] };
            keys = p.input(0, &pkt(&[4, 3, 1, r], &v)).0;
        }
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
        // a deeper reading raises that key's full travel
        let mut keys = None;
        for r in [3u8, 4, 0, 1, 2] {
            let v = if r == 2 { vec![(2, 4000)] } else { vec![] };
            keys = p.input(0, &pkt(&[4, 3, 1, r], &v)).0;
        }
        assert_eq!(keys.unwrap()[&0x1A], 1.0);
    }

    #[test]
    fn report_id_in_front_is_fine() {
        let mut p = SparkLink::default();
        p.start();
        let mut d = vec![0u8];
        d.extend(pkt(&[1, 2, 1], &[]));
        assert_eq!(p.input(0, &d).1[0].data[0], 3);
    }
}

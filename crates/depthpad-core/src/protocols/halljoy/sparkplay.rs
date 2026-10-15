//! AULA WIN 60 HE MAX/PRO, WIN 68 HE PRO/MAX, HERO 68 HE PRO and GravaStar Mercury V75 / Pro /
//! Lite (SparkPlayJoy firmware, 6x21 key matrix), from HallJoy's `aula_win60he_protocol.cpp`,
//! `aula_win60he_client.cpp` and `sparkplayjoy_layout.h`; and IROK MG75 Pro / NA87 Pro / ND63,
//! WLMOUSE Ying75, IYX MU68 Pro and CAROTMAS Mercury68 (JingTai firmware, the same frames), from
//! `mg75_pro_protocol.h`, `mg75_pro_backend.cpp`, `wlmouse_ying75_protocol.h` and
//! `jingtai_v1_profiles.h`.
//!
//! Vendor collection FFA0:0001, 64-byte reports without an ID. Requests and replies are frames:
//! 5C, payload length, command (replies set bit 7), checksum, payload; a reply can span up to
//! three reports. Setup: 0x01 sync gives the board number, 0x00/0x25 the travel range (µm),
//! 0x2B the factory key of every matrix position, two rows at a time. Depth is then polled:
//! 0x12/0x02 with half 1 (rows 0-2) or 2 (rows 3-5) returns each key's travel in µm.
//!
//! JingTai boards are known by USB ID and product name instead, with a fixed travel range and a
//! known key matrix (checked against the 0x2B rows where the maker's own client reads them).

use super::{le16, Matrix};
use crate::protocols::analog::{AnalogProtocol, Keys, Out};
use crate::protocols::halljoy::jingtai_tables::MODELS as JINGTAI;
use crate::protocols::UsbInfo;

/// A JingTai board.
pub struct JingTai {
    /// The name the layout catalogue knows it by.
    pub identity: &'static str,
    pub range_um: u32,
    /// Check the 0x2B factory rows against `actions` before reading travel.
    pub factory_check: bool,
    /// The Ying75's own client: travel replies are three reports whatever their length byte
    /// says, and nothing is checksummed.
    pub lenient: bool,
    /// (vid, pid, USB product name) it is sold as.
    pub usb: &'static [(u16, u16, &'static str)],
    /// Factory action of each of the 126 matrix positions (0xF001 is Fn).
    pub actions: &'static [u16],
}

fn decode_action(a: u16) -> u16 {
    match a {
        0xF001 => 0x409,
        0..=0xFF => a,
        _ => 0,
    }
}

/// The JingTai board with these USB IDs and product name.
pub fn find_jingtai(usb: &UsbInfo) -> Option<&'static JingTai> {
    JINGTAI.iter().find(|m| {
        m.usb.iter().any(|&(v, p, n)| v == usb.vendor_id && p == usb.product_id && n.eq_ignore_ascii_case(usb.product.trim()))
    })
}

const HEAD: u8 = 0x5C;
const COLS: usize = 21;
const ROWS: usize = 6;
const SYNC: u8 = 0x01;
const API: u8 = 0x00;
const DEFAULT_KEYS: u8 = 0x2B;
const MATRIX: u8 = 0x12;
const PRECISION_STROKE: u8 = 0x25;
const TRAVEL: u8 = 0x02;
/// HallJoy: one second for any reply.
const REPLY_SECS: f64 = 1.0;
const ATTEMPTS: u32 = 3;

/// The USB IDs and board numbers HallJoy admits.
pub const KNOWN: &[(u16, u16, u32)] = &[
    (0x1CA2, 0x1902, 0x0A02_1902), // AULA WIN 60 HE MAX / PRO
    (0x1CA2, 0x1901, 0x0602_1901), // AULA WIN 68 HE PRO / MAX
    (0x1CA5, 0x0409, 0x0605_0409), // AULA HERO 68 HE PRO
    (0x1CA5, 0x2201, 0x1605_2201), // GravaStar Mercury V75
    (0x1CA5, 0x2202, 0x1605_2202), // GravaStar Mercury V75 Pro
    (0x1CA2, 0x2201, 0x2E02_2201), // GravaStar Mercury V75 Lite
];

fn checksum(len: u8, cmd: u8, payload: &[u8]) -> u8 {
    let last = if len > 0 { payload[len as usize - 1] as u32 } else { 0 };
    ((0x35 + HEAD as u32 + len as u32 + cmd as u32 + last) & 0xFF) as u8
}

/// A request frame.
pub fn frame(cmd: u8, payload: &[u8]) -> Out {
    let mut d = vec![HEAD, payload.len() as u8, cmd, checksum(payload.len() as u8, cmd, payload)];
    d.extend_from_slice(payload);
    Out::write(0, 64, &d)
}

/// The analog code of a factory key: HID usages as they are, 01 is Fn.
fn key_code(v: u8) -> u16 {
    match v {
        0x01 => 0x409,
        0x04..=0xE7 => v as u16,
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Sync,
    Precision,
    Keys(u8), // first row of the pair asked for
    Travel(u8),
    Failed,
}

pub struct SparkPlay {
    vid: u16,
    pid: u16,
    jingtai: Option<&'static JingTai>,
    step: Step,
    sent_at: Option<f64>,
    now: f64,
    attempts: u32,
    /// Reply frame being collected.
    buf: Vec<u8>,
    cmd: u8,
    board: u32,
    max_um: u16,
    map: [u8; ROWS * COLS],
    /// Analog code of each matrix position.
    codes: [u16; ROWS * COLS],
    matrix: Matrix,
    reason: Option<String>,
}

impl SparkPlay {
    pub fn new(usb: &UsbInfo) -> Self {
        Self {
            vid: usb.vendor_id, pid: usb.product_id, jingtai: find_jingtai(usb), step: Step::Sync, sent_at: None, now: 0.0, attempts: 0, buf: Vec::new(), cmd: 0, board: 0,
            max_um: 0, map: [0; ROWS * COLS], codes: [0; ROWS * COLS], matrix: Matrix::default(), reason: None,
        }
    }

    fn ask(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.buf.clear();
        self.sent_at = None;
        let (cmd, payload): (u8, Vec<u8>) = match step {
            Step::Sync => (SYNC, vec![1, 2, 3, 4, 0xFF, 0xFF]),
            Step::Precision => (API, vec![PRECISION_STROKE, 0xFF, 0xFF]),
            Step::Keys(r) => (DEFAULT_KEYS, vec![0, r, r + 1]),
            Step::Travel(half) => (MATRIX, vec![TRAVEL, half, 0xFF, 0xFF]),
            Step::Failed => return Vec::new(),
        };
        self.cmd = cmd;
        vec![frame(cmd, &payload)]
    }

    fn fail(&mut self, why: &str) -> Vec<Out> {
        self.reason = Some(why.to_string());
        self.step = Step::Failed;
        Vec::new()
    }

    /// A complete reply's payload, once all its reports are in.
    fn collect(&mut self, d: &[u8]) -> Option<Vec<u8>> {
        if self.buf.is_empty() && (d.len() < 4 || d[0] != HEAD || d[2] != self.cmd | 0x80) {
            return None; // not the start of our reply
        }
        self.buf.extend_from_slice(&d[..d.len().min(64)]);
        let lenient = self.jingtai.is_some_and(|j| j.lenient);
        let total = if lenient && self.cmd == MATRIX { 132 } else { 4 + self.buf[1] as usize };
        if self.buf.len() < total {
            return None;
        }
        let (len, cmd, sum) = (self.buf[1], self.buf[2], self.buf[3]);
        let payload = self.buf[4..total].to_vec();
        self.buf.clear();
        (lenient || sum == checksum(len, cmd, &payload)).then_some(payload)
    }

    /// Where reading starts: JingTai boards skip the sync and range requests.
    fn first_step(&mut self) -> Vec<Out> {
        match self.jingtai {
            Some(j) => {
                self.max_um = j.range_um as u16;
                for (i, &a) in j.actions.iter().enumerate().take(ROWS * COLS) {
                    self.codes[i] = decode_action(a);
                }
                self.ask(if j.factory_check { Step::Keys(0) } else { Step::Travel(1) })
            }
            None => self.ask(Step::Sync),
        }
    }

    /// For a JingTai board: do the factory rows the keyboard reports match its matrix? (The
    /// Ying75 in macOS mode reports Left Win and Alt swapped.)
    fn factory_rows_match(&self, p: &[u8], r: usize) -> bool {
        let Some(j) = self.jingtai else { return false };
        let selector = |a: u16| if a == 0xF001 { 1 } else { a as u8 };
        let rows_match = |acts: &[u16]| {
            (0..COLS).all(|c| p[2 + c] == selector(acts[r * COLS + c]) && p[24 + c] == selector(acts[(r + 1) * COLS + c]))
        };
        let mut mac = j.actions.to_vec();
        mac.swap(106, 107);
        rows_match(j.actions) || (j.lenient && rows_match(&mac))
    }
}

impl AnalogProtocol for SparkPlay {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            return self.fail("the keyboard didn't complete analog setup");
        }
        self.first_step()
    }

    fn input(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let Some(p) = self.collect(d) else { return (None, Vec::new()) };
        let outs = match self.step {
            Step::Sync if p.len() == 60 && p[0] == 0 && p[26..31] == *b"App V" => {
                self.board = u32::from_le_bytes([p[1], p[2], p[3], p[4]]);
                let expected = KNOWN.iter().find(|k| (k.0, k.1) == (self.vid, self.pid)).map(|k| k.2);
                if expected.is_some_and(|b| b != self.board) {
                    return (None, self.fail(&format!("board {:08X} isn't the one this USB ID belongs to", self.board)));
                }
                self.ask(Step::Precision)
            }
            Step::Precision if p.len() == 7 && p[0] == 0 && p[1] == PRECISION_STROKE => {
                let (precision, min, max) = (p[2], le16(&p, 3), le16(&p, 5));
                if !(1..=100).contains(&precision) || min > max || !(500..=10000).contains(&max) {
                    return (None, self.fail("the keyboard reported an implausible travel range"));
                }
                self.max_um = max;
                self.ask(Step::Keys(0))
            }
            Step::Keys(r) if p.len() == 45 && p[0] == 0 && p[1] == r && p[23] == r + 1 && self.jingtai.is_some() => {
                if !self.factory_rows_match(&p, r as usize) {
                    return (None, self.fail("the keyboard's key matrix isn't the one expected for it"));
                }
                if r < 4 {
                    self.ask(Step::Keys(r + 2))
                } else {
                    self.attempts = 0;
                    self.ask(Step::Travel(1))
                }
            }
            Step::Keys(r) if p.len() == 45 && p[0] == 0 && p[1] == r && p[23] == r + 1 => {
                let r = r as usize;
                self.map[r * COLS..(r + 1) * COLS].copy_from_slice(&p[2..2 + COLS]);
                self.map[(r + 1) * COLS..(r + 2) * COLS].copy_from_slice(&p[24..24 + COLS]);
                if r < 4 {
                    self.ask(Step::Keys(r as u8 + 2))
                } else if self.map.iter().filter(|&&k| key_code(k) != 0).count() < 2 {
                    self.fail("the keyboard reported no keys")
                } else {
                    for (c, &k) in self.codes.iter_mut().zip(self.map.iter()) {
                        *c = key_code(k);
                    }
                    self.attempts = 0;
                    self.ask(Step::Travel(1))
                }
            }
            Step::Travel(half) if p.len() == 128 && p[0] == 0 && p[1] == TRAVEL => {
                let first_row = if half == 1 { 0 } else { 3 };
                for i in 0..3 * COLS {
                    let pos = first_row * COLS + i;
                    let um = le16(&p, 2 + 2 * i);
                    self.matrix.set(pos, self.codes[pos], um as f64 / self.max_um as f64);
                }
                let outs = self.ask(Step::Travel(if half == 1 { 2 } else { 1 }));
                // a full matrix after the second half
                return (if half == 2 { Some(self.matrix.keys()) } else { None }, outs);
            }
            // a reply that doesn't fit: ask again
            _ => self.ask(self.step),
        };
        (None, outs)
    }

    fn tick(&mut self, now: f64) -> Vec<Out> {
        self.now = now;
        let sent = *self.sent_at.get_or_insert(now);
        if self.step != Step::Failed && now - sent > REPLY_SECS {
            return self.start();
        }
        Vec::new()
    }

    fn identity(&self) -> Option<String> {
        match self.jingtai {
            Some(j) => Some(j.identity.to_string()),
            None => (self.board != 0).then(|| format!("{:08X}", self.board)),
        }
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| self.reason.clone().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reply frame split into 64-byte reports.
    fn reply(cmd: u8, payload: &[u8]) -> Vec<Vec<u8>> {
        let mut f = vec![HEAD, payload.len() as u8, cmd | 0x80, checksum(payload.len() as u8, cmd | 0x80, payload)];
        f.extend_from_slice(payload);
        f.resize(f.len().div_ceil(64) * 64, 0);
        f.chunks(64).map(|c| c.to_vec()).collect()
    }

    fn feed(p: &mut SparkPlay, reports: Vec<Vec<u8>>) -> (Option<Keys>, Vec<Out>) {
        let mut last = (None, Vec::new());
        for r in reports {
            last = p.input(0, &r);
        }
        last
    }

    #[test]
    fn checksum_matches_halljoy() {
        // 0x35 + 5C + len + cmd + last payload byte
        let o = frame(SYNC, &[1, 2, 3, 4, 0xFF, 0xFF]);
        assert_eq!(o.data[..4], [HEAD, 6, SYNC, ((0x35u32 + 0x5C + 6 + 1 + 0xFF) & 0xFF) as u8]);
    }

    #[test]
    fn win60_he_max_session() {
        let mut p = SparkPlay::new(&UsbInfo { vendor_id: 0x1CA2, product_id: 0x1902, ..Default::default() });
        assert_eq!(p.start()[0].data[2], SYNC);
        let mut sync = vec![0u8; 60];
        sync[1..5].copy_from_slice(&0x0A02_1902u32.to_le_bytes());
        sync[26..31].copy_from_slice(b"App V");
        let (_, outs) = feed(&mut p, reply(SYNC, &sync));
        assert_eq!(p.identity().as_deref(), Some("0A021902"));
        assert_eq!(outs[0].data[4], PRECISION_STROKE);
        // 10 µm precision, 10..3400 µm
        let (_, outs) = feed(&mut p, reply(API, &[0, PRECISION_STROKE, 10, 10, 0, 0x48, 0x0D]));
        assert_eq!(outs[0].data[2], DEFAULT_KEYS);
        for r in [0u8, 2, 4] {
            let mut k = vec![0u8; 45];
            (k[1], k[23]) = (r, r + 1);
            if r == 2 {
                k[2 + 2] = 0x1A; // W at row 2, column 2
            }
            if r == 4 {
                k[24 + 12] = 0x01; // Fn at row 5, column 12
            }
            feed(&mut p, reply(DEFAULT_KEYS, &k));
        }
        assert_eq!(p.step, Step::Travel(1));
        // half 1 (rows 0-2): W at 1700 µm
        let mut t = vec![0u8; 128];
        t[1] = TRAVEL;
        let i = 2 * COLS + 2;
        t[2 + 2 * i..4 + 2 * i].copy_from_slice(&1700u16.to_le_bytes());
        let (keys, outs) = feed(&mut p, reply(MATRIX, &t));
        assert!(keys.is_none());
        assert_eq!(outs[0].data[5], 2); // then half 2
        let mut t2 = vec![0u8; 128];
        t2[1] = TRAVEL;
        let (keys, _) = feed(&mut p, reply(MATRIX, &t2));
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
    }

    #[test]
    fn ying75_session() {
        let usb = UsbInfo { vendor_id: 0x36A7, product_id: 0xF887, product: "WLKB YING 75".into(), ..Default::default() };
        let mut p = SparkPlay::new(&usb);
        assert_eq!(p.identity().as_deref(), Some("YING75-36A7-F887"));
        let j = p.jingtai.unwrap();
        // factory rows first, in macOS mode (Left Win and Alt swapped)
        assert_eq!(p.start()[0].data[2], DEFAULT_KEYS);
        let mut mac = j.actions.to_vec();
        mac.swap(106, 107);
        let sel = |a: u16| if a == 0xF001 { 1 } else { a as u8 };
        for r in [0usize, 2, 4] {
            let mut k = vec![0u8; 45];
            (k[1], k[23]) = (r as u8, r as u8 + 1);
            for c in 0..COLS {
                k[2 + c] = sel(mac[r * COLS + c]);
                k[24 + c] = sel(mac[(r + 1) * COLS + c]);
            }
            feed(&mut p, reply(DEFAULT_KEYS, &k));
        }
        assert_eq!(p.step, Step::Travel(1));
        // lenient travel: a wrong checksum and length byte are fine; W is position 44, 1650 of 3300 µm
        let mut t = vec![0u8; 128];
        t[1] = TRAVEL;
        let w = j.actions.iter().position(|&a| a == 0x1A).unwrap();
        assert_eq!(w, 44);
        t[2 + 2 * w..4 + 2 * w].copy_from_slice(&1650u16.to_le_bytes());
        let mut frames = reply(MATRIX, &t);
        frames[0][1] = 0;
        frames[0][3] ^= 0xFF;
        let (_, _) = feed(&mut p, frames);
        let mut t2 = vec![0u8; 128];
        t2[1] = TRAVEL;
        let (keys, _) = feed(&mut p, reply(MATRIX, &t2));
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
    }

    #[test]
    fn jingtai_by_product_name() {
        // the ND63 and its cyan edition share USB IDs and a matrix, not their travel
        let nd63 = UsbInfo { vendor_id: 7247, product_id: 61064, product: "IROK ND63".into(), ..Default::default() };
        let cyan = UsbInfo { product: "IROK ND63 CYAN".into(), ..nd63.clone() };
        assert_eq!(find_jingtai(&nd63).unwrap().range_um, 4000);
        assert_eq!(find_jingtai(&cyan).unwrap().range_um, 3600);
        assert!(find_jingtai(&UsbInfo { product: "SOMETHING".into(), ..nd63 }).is_none());
    }

    #[test]
    fn wrong_board_for_the_usb_id() {
        let mut p = SparkPlay::new(&UsbInfo { vendor_id: 0x1CA2, product_id: 0x1902, ..Default::default() });
        p.start();
        let mut sync = vec![0u8; 60];
        sync[1..5].copy_from_slice(&0x1605_2201u32.to_le_bytes());
        sync[26..31].copy_from_slice(b"App V");
        feed(&mut p, reply(SYNC, &sync));
        assert!(p.problem().is_some());
    }

    #[test]
    fn silence_restarts_then_gives_up() {
        let mut p = SparkPlay::new(&UsbInfo { vendor_id: 0x1CA2, product_id: 0x1902, ..Default::default() });
        p.start();
        let mut t = 0.0;
        for _ in 0..100 {
            t += 0.25;
            p.tick(t);
        }
        assert!(p.problem().is_some());
    }
}

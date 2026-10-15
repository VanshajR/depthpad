//! AULA WIN 60 HE / WIN 68 HE / KP-TE153 and Redragon K673, K686 and K617 HE (iLLumiPC firmware),
//! from HallJoy's `aula_w669_protocol.cpp` and `aula_w669_backend.cpp`; and the IROK NA87 (M484
//! controller, the same frames), from `irok_nd75_protocol.cpp`, `irok_na87_protocol.h` and
//! `irok_na87_backend.cpp`; and the AJAZZ AK820 MAX HE wired RGB revision (M484, "SG8994HERGB"), from
//! `irok_na87_backend.cpp` (RunAjazz) and `ajazz_raw_limits.h`.
//!
//! Vendor collection FF1B:0091, 64-byte reports with ID 1. Setup: device info (0x0D) gives the
//! firmware product name, which picks the factory key matrix; 0x21/0x04 gives the travel range;
//! 0x18/0x80 sends the key map in ten fragments (non-zero records override the factory map).
//! Then 0x21/0x02 subscribes the mapped matrix positions and the keyboard sends one event per
//! key change (row, column, travel).
//!
//! The NA87 answers 0x0D with controller "M484" and product "GK8260HERGB"; its travel scale
//! (0x21/18/04) must be 40, its key map comes from 0x10 in six rows of 22, and its events carry
//! an 8-bit travel of 0-40.
//!
//! The AK820 MAX HE (M484, "SG8994HERGB", firmware V1.13.17 only) reads its key map the same way
//! (keys may repeat), then 0x23 .. 1 streams raw sensor rows (row 1-6, 22 big-endian values) about
//! a thousand times a second. Each sensor's range is learned from its own readings: the highest
//! (median of three) is released, the lowest pressed, seeded with HallJoy's measured bottoms.

use super::{le16, Deadline, Matrix};
use crate::protocols::analog::{AnalogProtocol, Keys, Out};

const ID: u8 = 1;
const LEN: usize = 63;
const ROWS: usize = 6;
const COLS: usize = 22;
const SLOTS: usize = ROWS * COLS;
const INFO: u8 = 0x0D;
const ANALOG: u8 = 0x21;
const KEYMAP: u8 = 0x18;
/// HallJoy: 400 ms for the identity, 1.2 s for the other setup steps.
const IDENTITY_SECS: f64 = 0.4;
const STEP_SECS: f64 = 1.2;
/// Setup attempts before giving up on the keyboard.
const ATTEMPTS: u32 = 3;

type Map = [u8; SLOTS];

fn request(command: u8, at: &[(usize, u8)]) -> Out {
    let mut o = Out::write(ID, LEN, &[command]);
    for &(i, v) in at {
        o.data[i - 1] = v;
    }
    o
}

/// The fields of the CSV text in the 0x0D reply (controller first, product fifth).
pub fn decode_info(d: &[u8]) -> Option<Vec<String>> {
    if d.len() < LEN || d[0] != INFO || d[1] != 0 || d[3] != 0 || d[4] <= 5 || d[4] as usize >= 64 {
        return None;
    }
    let text = &d[5..d[4] as usize];
    let text = &text[..text.iter().position(|&b| b == 0).unwrap_or(text.len())];
    if text.iter().any(|&b| b != b',' && !(0x20..=0x7E).contains(&b)) {
        return None;
    }
    Some(String::from_utf8_lossy(text).split(',').map(str::to_string).collect())
}

/// The NA87's key map, one 0x10 row reply at a time: Some(row) for a row it stored.
pub fn decode_na87_row(d: &[u8], map: &mut Map) -> Option<usize> {
    if d.len() < 27 || d[0] != 0x10 || d[1] != 0 || d[4] != 22 {
        return None;
    }
    let row = match (d[2], d[3]) {
        (0, r @ 1..=5) => r as usize - 1,
        (255, 255) => 5, // the last row has an end marker instead of a number
        _ => return None,
    };
    map[row * COLS..(row + 1) * COLS].copy_from_slice(&d[5..5 + COLS]);
    Some(row)
}

/// A usable NA87 map: plain keyboard usages (and Fn, 0xFA), each once, 60-110 keys.
fn na87_map_ok(map: &Map) -> bool {
    let mut seen = [false; 256];
    let mut count = 0;
    for &h in map.iter().filter(|&&h| h != 0) {
        if h < 4 || (h > 0xE7 && h != 0xFA) || seen[h as usize] {
            return false;
        }
        seen[h as usize] = true;
        count += 1;
    }
    (60..=110).contains(&count)
}

/// The firmware product name: field 4 of the CSV text in the 0x0D reply.
pub fn decode_product(d: &[u8]) -> Option<String> {
    // HallJoy indices: r[1]=0x0D, r[2]=0 (success), r[4]=0, r[5]=end of text, text from r[6]
    if d.len() < LEN || d[0] != INFO || d[1] != 0 || d[3] != 0 || d[4] <= 5 || d[4] as usize >= 64 {
        return None;
    }
    let end = d[4] as usize; // exclusive end, HallJoy's r[5]+1 minus the ID byte
    let (mut field, mut product) = (0, String::new());
    for &b in d.get(5..end)? {
        if b == 0 {
            break;
        }
        if b == b',' {
            field += 1;
            if field > 4 {
                break;
            }
            continue;
        }
        if !(0x20..=0x7E).contains(&b) {
            return None;
        }
        if field == 4 {
            if product.len() + 1 >= 32 {
                return None;
            }
            product.push(b as char);
        }
    }
    (field >= 4 && !product.is_empty()).then_some(product)
}

/// Travel range from the 0x21/0x04 reply: (maximum, release floor).
pub fn decode_travel(d: &[u8]) -> Option<(u16, u16)> {
    if d.len() < LEN || d[0] != ANALOG || d[5] != 0x04 || d[4] < 6 || d[6] == 0 || d[9] == 0 {
        return None;
    }
    let max = d[6] as u16 | (d[9] as u16) << 8;
    if !(32..=10000).contains(&max) {
        return None;
    }
    // the keyboard's own minimum actuation travel: at or below it a key counts as released
    // (ignored when implausible, 20% of the range or more)
    let floor = d[10] as u16;
    Some((max, if floor as u32 * 5 < max as u32 { floor } else { 0 }))
}

/// One key-map fragment: applies its explicit records to `map`, returns the fragment number.
pub fn decode_map_fragment(d: &[u8], map: &mut Map) -> Option<usize> {
    if d.len() < LEN || d[0] != KEYMAP || d[1] != 0x80 {
        return None;
    }
    let fragment = (d[2] as usize) << 8 | d[3] as usize;
    if fragment >= 10 {
        return None;
    }
    let expected = if fragment < 9 { 56 } else { 24 };
    if d[4] as usize != expected || 5 + expected > d.len() {
        return None;
    }
    for i in 0..expected / 4 {
        let (code1, hid) = (d[5 + i * 4], d[6 + i * 4]);
        // zero records inherit the factory key; ordinary ones carry a HID usage in byte 1,
        // advanced/macro ones (no usage) are left out
        if code1 != 0 && code1 != 0xFF {
            map[fragment * 14 + i] = if hid != 0 && hid != 0xFF { hid } else { 0 };
        }
    }
    Some(fragment)
}

/// A live key event: (matrix position, raw travel).
pub fn decode_event(d: &[u8]) -> Option<(usize, u16)> {
    if d.len() < 10 || d[0] != ANALOG || d[5] != 0x01 || d[4] < 3 {
        return None;
    }
    let (row, col) = (d[6] as usize, d[7] as usize);
    (row < ROWS && col < COLS).then(|| (row * COLS + col, le16(d, 8)))
}

fn subscription(map: &Map) -> Out {
    let mut mask = [0u8; COLS];
    for (pos, &hid) in map.iter().enumerate() {
        if hid != 0 {
            mask[pos % COLS] |= 1 << (pos / COLS);
        }
    }
    let mut o = request(ANALOG, &[(5, 0x18), (6, 0x02)]);
    o.data[6..6 + COLS].copy_from_slice(&mask);
    o
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Identity,
    Travel,
    Map,
    Live,
    Failed,
}

/// IROK NA87 (M484) rather than iLLumiPC.
const NA87_PRODUCT: &str = "GK8260HERGB";
/// AJAZZ AK820 MAX HE, wired RGB revision (M484).
const AK820_PRODUCT: &str = "SG8994HERGB";
const AK820_FIRMWARE: &str = "V1.13.17";
/// The AK820's sensors (bit per column, per row): the firmware map is larger than the board.
const AK820_SENSORS: [u32; 6] = [0xffbd, 0xdfff, 0xdfff, 0xdffd, 0xeffd, 0xfc47];
/// A raw row going quiet this long (or one never arriving) means the stream stopped.
const AK820_ROW_SECS: f64 = 0.1;
const AK820_FIRST_ROW_SECS: f64 = 1.2;

fn ak820_sensor(pos: usize) -> bool {
    pos < SLOTS && AK820_SENSORS[pos / COLS] & (1 << (pos % COLS)) != 0
}

/// One AK820 sensor's learned range.
#[derive(Clone, Copy, Default)]
struct Range {
    recent: [u16; 3],
    next: usize,
    samples: u32,
    rest: u16,
    full: u16,
}

impl Range {
    /// HallJoy's measured bottom-out readings (other keys: their median, 1353).
    fn initial_full(pos: usize) -> u16 {
        match pos {
            46 | 68 => 1353, 69 => 1355, 70 => 1366, 71 => 1431, 72 => 1341, 90 => 1345, 91 => 1317,
            92 => 1293, 93 => 1382, 94 => 1301, 121 => 1289, _ => 1353,
        }
    }
    /// Depth from a raw reading, once the range is wide enough to trust.
    fn depth(&mut self, pos: usize, raw: u16) -> Option<f64> {
        if raw == 0 {
            return None;
        }
        if self.samples == 0 {
            (self.recent, self.rest, self.full) = ([raw; 3], raw, Self::initial_full(pos));
        }
        self.recent[self.next] = raw;
        self.next = (self.next + 1) % 3;
        self.samples += 1;
        let mut a = self.recent;
        a.sort_unstable();
        let v = a[1]; // a median of three drops single spikes
        self.rest = self.rest.max(v);
        self.full = self.full.min(v);
        if self.rest as u32 <= self.full as u32 + 32 {
            return None; // a key held since start: too little range yet
        }
        Some(if v >= self.rest { 0.0 } else if v <= self.full { 1.0 } else { (self.rest - v) as f64 / (self.rest - self.full) as f64 })
    }
}

pub struct AulaW669 {
    step: Step,
    deadline: Deadline,
    attempts: u32,
    product: Option<String>,
    max: u16,
    floor: u16,
    map: Map,
    received: [bool; 10],
    matrix: Matrix,
    na87: bool,
    /// AJAZZ AK820 MAX HE (wired RGB): the NA87's setup, then raw rows.
    ak820: bool,
    m484_unknown: bool,
    ranges: Vec<Range>,
    row_seen: [Option<f64>; ROWS],
    live_since: Option<f64>,
    now: f64,
}

impl Default for AulaW669 {
    fn default() -> Self {
        Self {
            step: Step::Identity,
            deadline: Deadline::default(),
            attempts: 0,
            product: None,
            max: 0,
            floor: 0,
            map: [0; SLOTS],
            received: [false; 10],
            matrix: Matrix::default(),
            na87: false,
            ak820: false,
            m484_unknown: false,
            ranges: vec![Range::default(); SLOTS],
            row_seen: [None; ROWS],
            live_since: None,
            now: 0.0,
        }
    }
}

impl AulaW669 {
    fn begin(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.deadline.start();
        match step {
            Step::Identity => vec![request(INFO, &[])],
            Step::Travel if self.na87 => vec![request(ANALOG, &[(5, 0x18), (6, 0x04)])],
            Step::Travel => vec![request(ANALOG, &[(6, 0x04)])],
            Step::Map if self.na87 => {
                self.map = [0; SLOTS];
                self.received = [false; 10];
                vec![request(0x10, &[])]
            }
            Step::Map => {
                self.map = factory_map(self.product.as_deref().unwrap_or(""));
                self.received = [false; 10];
                vec![request(KEYMAP, &[(2, 0x80)])]
            }
            Step::Live if self.ak820 => {
                self.deadline.stop();
                (self.ranges, self.row_seen, self.live_since) = (vec![Range::default(); SLOTS], [None; ROWS], None);
                vec![request(0x23, &[(6, 1)])]
            }
            Step::Live if self.na87 => {
                self.deadline.stop();
                vec![request(ANALOG, &[(5, 0x18), (6, 0x03)]), subscription(&self.map)]
            }
            Step::Live => {
                self.deadline.stop();
                vec![subscription(&self.map)]
            }
            Step::Failed => {
                self.deadline.stop();
                Vec::new()
            }
        }
    }
}

impl AnalogProtocol for AulaW669 {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            return self.begin(Step::Failed);
        }
        self.begin(Step::Identity)
    }

    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if id != ID {
            return (None, Vec::new());
        }
        match self.step {
            Step::Identity => {
                if let Some(f) = decode_info(d) {
                    let m484 = f.len() >= 5 && f[0] == "M484";
                    self.ak820 = m484 && f[4] == AK820_PRODUCT && f.get(5).is_some_and(|v| v.trim_end() == AK820_FIRMWARE);
                    self.na87 = m484 && (f[4] == NA87_PRODUCT || self.ak820);
                    // other M484 boards and firmware: HallJoy sends them nothing more, nor do we
                    if m484 && !self.na87 {
                        self.product = Some(format!("{} {}", f[4], f.get(5).map_or("", |v| v.trim_end())));
                        self.m484_unknown = true;
                        return (None, self.begin(Step::Failed));
                    }
                }
                if let Some(p) = decode_product(d) {
                    self.product = Some(p);
                    return (None, self.begin(Step::Travel));
                }
            }
            // the NA87's scale must be 0-40
            Step::Travel if self.na87 => {
                if d.len() >= LEN && d[0] == ANALOG && d[4] >= 2 && d[5] == 0x04 {
                    if d[6] != 40 {
                        return (None, self.begin(Step::Failed));
                    }
                    (self.max, self.floor) = (40, 0);
                    return (None, self.begin(Step::Map));
                }
            }
            Step::Map if self.na87 => {
                if let Some(row) = decode_na87_row(d, &mut self.map) {
                    self.received[row] = true;
                    if self.received[..6].iter().all(|&r| r) {
                        if self.ak820 {
                            // keys may repeat; only real sensors count, Fn (FA) as Fn
                            for (pos, h) in self.map.iter_mut().enumerate() {
                                if !ak820_sensor(pos) {
                                    *h = 0;
                                }
                            }
                            if self.map.iter().all(|&h| h == 0) {
                                return (None, self.begin(Step::Failed));
                            }
                            self.attempts = 0;
                            return (None, self.begin(Step::Live));
                        }
                        if !na87_map_ok(&self.map) {
                            return (None, self.begin(Step::Failed));
                        }
                        self.attempts = 0;
                        return (Some(Keys::new()), self.begin(Step::Live));
                    }
                }
            }
            Step::Live if self.ak820 => {
                // a raw row: 23 00 00 <row 1-6> 2C, then 22 big-endian values
                if d.len() >= 5 + COLS * 2 && d[0] == 0x23 && d[1] == 0 && d[2] == 0 && (1..=6).contains(&d[3]) && d[4] == 44 {
                    let row = d[3] as usize - 1;
                    for col in 0..COLS {
                        let pos = row * COLS + col;
                        if !ak820_sensor(pos) {
                            continue;
                        }
                        let raw = u16::from_be_bytes([d[5 + col * 2], d[6 + col * 2]]);
                        let depth = self.ranges[pos].depth(pos, raw).unwrap_or(0.0);
                        let code = match self.map[pos] { 0xFA => 0x409, h => h as u16 };
                        self.matrix.set(pos, code, depth);
                    }
                    self.row_seen[row] = Some(self.now);
                    // keys only once every row is live
                    if self.row_seen.iter().all(Option::is_some) {
                        return (Some(self.matrix.keys()), Vec::new());
                    }
                }
            }
            Step::Live if self.na87 => {
                // an event: 21 00 00 00 03 01 row column travel
                if d.len() >= 9 && d[0] == ANALOG && d[1..4] == [0, 0, 0] && d[4] == 3 && d[5] == 1 {
                    let (row, col, travel) = (d[6] as usize, d[7] as usize, d[8]);
                    if row < ROWS && col < COLS && travel <= 40 {
                        let pos = row * COLS + col;
                        self.matrix.set(pos, self.map[pos] as u16, travel as f64 / 40.0);
                        return (Some(self.matrix.keys()), Vec::new());
                    }
                }
            }
            Step::Travel => {
                if let Some((max, floor)) = decode_travel(d) {
                    (self.max, self.floor) = (max, floor);
                    return (None, self.begin(Step::Map));
                }
            }
            Step::Map => {
                if decode_map_fragment(d, &mut self.map).is_some_and(|f| {
                    self.received[f] = true;
                    self.received.iter().all(|&r| r)
                }) {
                    if self.map.iter().filter(|&&h| h != 0).count() < 20 {
                        return (None, self.begin(Step::Failed));
                    }
                    self.attempts = 0;
                    return (Some(Keys::new()), self.begin(Step::Live));
                }
            }
            Step::Live => {
                if let Some((pos, raw)) = decode_event(d) {
                    let raw = if raw <= self.floor { 0 } else { raw };
                    self.matrix.set(pos, self.map[pos] as u16, raw as f64 / self.max as f64);
                    return (Some(self.matrix.keys()), Vec::new());
                }
            }
            Step::Failed => {}
        }
        (None, Vec::new())
    }

    fn tick(&mut self, now: f64) -> Vec<Out> {
        self.now = now;
        if self.step == Step::Live && self.ak820 {
            let since = *self.live_since.get_or_insert(now);
            // a dead row must not keep its keys pressed while the others go on
            let stale = self.row_seen.iter().any(|seen| match seen {
                Some(t) => now - t > AK820_ROW_SECS,
                None => now - since > AK820_FIRST_ROW_SECS,
            });
            return if stale { self.start() } else { Vec::new() };
        }
        match self.step {
            // no firmware name in time: carry on without it, as HallJoy does
            Step::Identity if self.deadline.expired(now, IDENTITY_SECS) => self.begin(Step::Travel),
            Step::Travel | Step::Map if self.deadline.expired(now, STEP_SECS) => self.start(),
            _ => Vec::new(),
        }
    }

    fn identity(&self) -> Option<String> {
        self.product.clone()
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| match &self.product {
            Some(p) if self.m484_unknown => format!("\"{p}\" (M484) isn't a model and firmware Depthpad knows"),
            Some(p) if !self.na87 && factory_map(p).iter().all(|&h| h == 0) => {
                format!("firmware \"{p}\" isn't one Depthpad knows the key matrix of yet")
            }
            _ => "the keyboard didn't complete analog setup".to_string(),
        })
    }

    fn event_driven(&self) -> bool {
        self.step == Step::Live && !self.ak820
    }

    fn stop(&mut self) -> Vec<Out> {
        if self.step == Step::Live && self.ak820 {
            vec![request(0x23, &[])]
        } else if self.step == Step::Live && self.na87 {
            vec![request(ANALOG, &[(5, 0x18), (6, 0x03)])]
        } else if self.step == Step::Live {
            vec![request(ANALOG, &[(6, 0x03)])]
        } else {
            Vec::new()
        }
    }
}

/// The factory key matrix for a firmware product name (row-major 6x22 sensor positions -> HID
/// usage; Fn is 0xFA). Unknown names get an empty map: only explicit key-map records count.
pub fn factory_map(product: &str) -> Map {
    let p: String = product.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    match p.as_str() {
        "SI2825HEARGB" | "SI2825KRT12HEARGB" | "SI2825KR-AHEARGB" | "SI2825KZHEARGB" => win60(),
        "SI2828HEARGB" | "SI2828KZHEARGB" => win68(),
        "SI2851UKKZHEARGB" => kp_te153_uk(),
        "7272BRHEXYXK673JCARGB" => k673_br(),
        "7272UKHEXYXBJCARGB" => k673_uk(),
        "7272USHEXYXK673JCARGB" => k673_us(),
        "7153USHEXYXCPARGB" => K617_US,
        "7153BRHEXYXCPARGB" => K617_BR,
        "7431USHEXYXK686JCARGB" => K686_US,
        "7431BRHEXYXJCARGB" => K686_BR,
        "7431UKHEXYXBJCARGB" => K686_UK,
        _ => [0; SLOTS],
    }
}

fn win60() -> Map {
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x29, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, //
        0x2d, 0x2e, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x2b, 0x14, 0x1a, 0x08, 0x15, 0x17, 0x1c, 0x18, 0x0c, 0x12, 0x13, //
        0x2f, 0x30, 0x00, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x39, 0x00, 0x04, 0x16, 0x07, 0x09, 0x0a, 0x0b, 0x0d, 0x0e, 0x0f, //
        0x33, 0x34, 0x00, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0xe1, 0x00, 0x1d, 0x1b, 0x06, 0x19, 0x05, 0x11, 0x10, 0x36, 0x37, //
        0x38, 0xe5, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0xe0, 0xe3, 0xe2, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0xe6, 0x65, //
        0xe4, 0xfa, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

fn win68() -> Map {
    let mut m = win60();
    for (i, v) in [
        (38, 0x49), (60, 0x4c), (82, 0x4b), (100, 0x00), (101, 0xe5), (103, 0x52), (104, 0x4e),
        (119, 0xe6), (120, 0x00), (121, 0xfa), (122, 0xe4), (124, 0x50), (125, 0x51), (126, 0x4f),
    ] {
        m[i] = v;
    }
    m
}

fn kp_te153_uk() -> Map {
    let mut m = win68();
    for (i, v) in [(38, 0x4a), (58, 0x28), (79, 0x32), (80, 0x00), (89, 0x64), (99, 0x87)] {
        m[i] = v;
    }
    m
}

fn k673_br() -> Map {
    [
        0x29, 0x00, 0x3a, 0x3b, 0x3c, 0x3d, 0x00, 0x3e, 0x3f, 0x40, 0x41, //
        0x42, 0x43, 0x44, 0x45, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x35, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, //
        0x2d, 0x2e, 0x00, 0x2a, 0x49, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x2b, 0x14, 0x1a, 0x08, 0x15, 0x17, 0x1c, 0x18, 0x0c, 0x12, 0x13, //
        0x2f, 0x30, 0x00, 0x00, 0x4c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x39, 0x00, 0x04, 0x16, 0x07, 0x09, 0x0a, 0x0b, 0x0d, 0x0e, 0x0f, //
        0x33, 0x34, 0x32, 0x28, 0x4b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0xe1, 0x64, 0x1d, 0x1b, 0x06, 0x19, 0x05, 0x11, 0x10, 0x36, 0x37, //
        0x38, 0x87, 0x00, 0x52, 0x4e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0xe0, 0xe3, 0xe2, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0xe6, //
        0x00, 0xe4, 0x50, 0x51, 0x4f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

fn k673_uk() -> Map {
    let mut m = k673_br();
    m[79] = 0x31;
    m[100] = 0x00;
    m[101] = 0xe5;
    m
}

fn k673_us() -> Map {
    let mut m = k673_uk();
    m[58] = 0x31;
    m[79] = 0x00;
    m[89] = 0x00;
    m
}

#[rustfmt::skip]
const K617_US: Map = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,41,30,31,32,33,34,35,36,37,38,39,45,46,0,42,0,0,0,0,0,0,0,43,20,26,8,21,23,28,24,12,18,19,47,48,0,49,0,0,0,0,0,0,0,57,0,4,22,7,9,10,11,13,14,15,51,52,0,40,0,0,0,0,0,0,0,225,0,29,27,6,25,5,17,16,54,55,56,229,0,0,0,0,0,0,0,0,0,224,227,226,0,0,0,44,0,0,230,250,101,228,0,0,0,0,0,0,0,0,0];
#[rustfmt::skip]
const K617_BR: Map = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,41,30,31,32,33,34,35,36,37,38,39,45,46,0,42,0,0,0,0,0,0,0,43,20,26,8,21,23,28,24,12,18,19,47,48,0,0,0,0,0,0,0,0,0,57,0,4,22,7,9,10,11,13,14,15,51,52,50,40,0,0,0,0,0,0,0,225,100,29,27,6,25,5,17,16,54,55,56,135,0,229,0,0,0,0,0,0,0,224,227,226,0,0,0,44,0,0,230,250,101,0,0,228,0,0,0,0,0,0,0];
#[rustfmt::skip]
const K686_US: Map = [41,0,58,59,60,61,62,63,64,65,66,67,68,69,76,73,75,78,0,0,0,0,53,30,31,32,33,34,35,36,37,38,39,45,46,0,42,83,84,85,86,0,0,0,43,20,26,8,21,23,28,24,12,18,19,47,48,0,49,95,96,97,87,0,0,0,57,0,4,22,7,9,10,11,13,14,15,51,52,0,40,92,93,94,0,0,0,0,225,0,29,27,6,25,5,17,16,54,55,56,229,0,82,89,90,91,88,0,0,0,224,227,226,0,0,0,44,0,0,230,250,228,0,80,81,79,98,99,0,0,0,0];
#[rustfmt::skip]
const K686_BR: Map = [41,0,58,59,60,61,62,63,64,65,66,67,68,69,76,73,75,78,0,0,0,0,53,30,31,32,33,34,35,36,37,38,39,45,46,0,42,83,84,85,86,0,0,0,43,20,26,8,21,23,28,24,12,18,19,47,48,0,0,95,96,97,87,0,0,0,57,0,4,22,7,9,10,11,13,14,15,51,52,50,40,92,93,94,0,0,0,0,225,100,29,27,6,25,5,17,16,54,55,56,229,0,82,89,90,91,88,0,0,0,224,227,226,0,0,0,44,0,0,230,250,228,0,80,81,79,98,99,0,0,0,0];
#[rustfmt::skip]
const K686_UK: Map = [41,0,58,59,60,61,62,63,64,65,66,67,68,69,76,73,75,78,0,0,0,0,53,30,31,32,33,34,35,36,37,38,39,45,46,0,42,83,84,85,86,0,0,0,43,20,26,8,21,23,28,24,12,18,19,47,48,0,0,95,96,97,87,0,0,0,57,0,4,22,7,9,10,11,13,14,15,51,52,49,40,92,93,94,0,0,0,0,225,100,29,27,6,25,5,17,16,54,55,56,229,0,82,89,90,91,88,0,0,0,224,227,226,0,0,0,44,0,0,230,250,228,0,80,81,79,98,99,0,0,0,0];

#[cfg(test)]
mod tests {
    use super::*;

    /// A reply as the keyboard sends it, without the report ID: `at` uses HallJoy's indices.
    fn reply(at: &[(usize, u8)]) -> Vec<u8> {
        let mut d = vec![0u8; LEN];
        for &(i, v) in at {
            d[i - 1] = v;
        }
        d
    }

    fn info(product: &str) -> Vec<u8> {
        let text = format!("A,B,C,D,{product}");
        let mut d = reply(&[(1, INFO), (5, (5 + text.len()) as u8)]);
        d[5..5 + text.len()].copy_from_slice(text.as_bytes());
        d
    }

    fn fragment(n: usize, records: &[(usize, u8, u8)]) -> Vec<u8> {
        let len = if n < 9 { 56 } else { 24 };
        let mut d = reply(&[(1, KEYMAP), (2, 0x80), (3, (n >> 8) as u8), (4, n as u8), (5, len)]);
        for &(i, c, h) in records {
            d[5 + i * 4] = c;
            d[6 + i * 4] = h;
        }
        d
    }

    #[test]
    fn product_name_from_device_info() {
        assert_eq!(decode_product(&info("SI2828HEARGB")).as_deref(), Some("SI2828HEARGB"));
        assert_eq!(decode_product(&info("")), None);
        assert_eq!(factory_map("si2828 hea rgb")[121], 0xfa); // normalised, Fn at 121
    }

    #[test]
    fn full_session() {
        let mut p = AulaW669::default();
        let first = p.start();
        assert_eq!((first[0].report_id, first[0].data[0]), (1, INFO));
        let (_, out) = p.input(1, &info("SI2825HEARGB"));
        assert_eq!((out[0].data[0], out[0].data[5]), (ANALOG, 0x04));
        assert_eq!(p.identity().as_deref(), Some("SI2825HEARGB"));
        // max 0x0190 = 400 (low byte r[7], high byte r[10]), floor 10
        let (_, out) = p.input(1, &reply(&[(1, ANALOG), (5, 6), (6, 0x04), (7, 0x90), (10, 0x01), (11, 10)]));
        assert_eq!((out[0].data[0], out[0].data[1]), (KEYMAP, 0x80));
        assert_eq!((p.max, p.floor), (400, 10));
        // fragment 1, record 8 is position 22 (Esc): remapped to F13
        for n in 0..10 {
            let recs: &[(usize, u8, u8)] = if n == 1 { &[(8, 1, 0x68)] } else { &[] };
            let (keys, out) = p.input(1, &fragment(n, recs));
            if n < 9 {
                assert!(keys.is_none() && out.is_empty());
            } else {
                assert_eq!(keys, Some(Keys::new()));
                assert_eq!((out[0].data[0], out[0].data[5]), (ANALOG, 0x02));
                // W (0x1a) is at position 46 = row 2, column 2: mask[2] bit 2
                assert_eq!(out[0].data[6 + 2] & 0b100, 0b100);
            }
        }
        assert_eq!(p.map[22], 0x68);
        assert!(p.event_driven());
        // W (row 2, column 2) half way: travel 200 of 400
        let ev = |row: u8, col: u8, t: u16| reply(&[(1, ANALOG), (5, 4), (6, 0x01), (7, row), (8, col), (9, t as u8), (10, (t >> 8) as u8)]);
        let (keys, _) = p.input(1, &ev(2, 2, 200));
        assert_eq!(keys.unwrap().get(&0x1a), Some(&0.5));
        // at or under the release floor counts as released
        let (keys, _) = p.input(1, &ev(2, 2, 10));
        assert!(keys.unwrap().is_empty());
        assert_eq!(p.stop()[0].data[5], 0x03);
    }

    #[test]
    fn irok_na87_session() {
        let mut p = AulaW669::default();
        p.start();
        let text = "M484,x,y,z,GK8260HERGB,V1.02";
        let mut d = reply(&[(1, INFO), (5, (5 + text.len()) as u8)]);
        d[5..5 + text.len()].copy_from_slice(text.as_bytes());
        let (_, out) = p.input(1, &d);
        assert!(p.na87);
        assert_eq!(p.identity().as_deref(), Some("GK8260HERGB"));
        assert_eq!((out[0].data[0], out[0].data[4], out[0].data[5]), (ANALOG, 0x18, 0x04));
        // scale 40
        let (_, out) = p.input(1, &reply(&[(1, ANALOG), (5, 2), (6, 0x04), (7, 40)]));
        assert_eq!(out[0].data[0], 0x10);
        // six rows: a 60%-ish map with W at row 2, column 2; Fn (0xFA) on the last row
        let usages: Vec<u8> = (4..=0x45).collect();
        let mut next = usages.iter();
        for row in 0..6u8 {
            let (a, b) = if row == 5 { (255, 255) } else { (0, row + 1) };
            let mut r = reply(&[(1, 0x10), (3, a), (4, b), (5, 22)]);
            for c in 0..COLS {
                r[5 + c] = match (row, c) {
                    (2, 2) => 0x1A,
                    (5, 10) => 0xFA,
                    (_, 0..=11) => *next.find(|&&u| u != 0x1A).unwrap_or(&0),
                    _ => 0,
                };
            }
            let (keys, out) = p.input(1, &r);
            if row == 5 {
                assert_eq!(keys, Some(Keys::new()));
                assert_eq!((out[0].data[5], out[1].data[5]), (0x03, 0x02)); // unsubscribe, subscribe
            }
        }
        // W at 20 of 40
        let ev = reply(&[(1, ANALOG), (5, 3), (6, 1), (7, 2), (8, 2), (9, 20)]);
        assert_eq!(p.input(1, &ev).0.unwrap()[&0x1A], 0.5);
    }

    fn m484_info(product: &str, firmware: &str) -> Vec<u8> {
        let text = format!("M484,x,y,z,{product},{firmware}");
        let mut d = reply(&[(1, INFO), (5, (5 + text.len()) as u8)]);
        d[5..5 + text.len()].copy_from_slice(text.as_bytes());
        d
    }

    #[test]
    fn ajazz_ak820_session() {
        let mut p = AulaW669::default();
        p.start();
        // only the exact firmware (space-padded) counts
        p.input(1, &m484_info("SG8994HERGB", "V1.13.17  "));
        assert!(p.ak820 && p.na87);
        assert_eq!(p.identity().as_deref(), Some("SG8994HERGB"));
        p.input(1, &reply(&[(1, ANALOG), (5, 2), (6, 0x04), (7, 40)]));
        for row in 0..6u8 {
            let (a, b) = if row == 5 { (255, 255) } else { (0, row + 1) };
            let mut r = reply(&[(1, 0x10), (3, a), (4, b), (5, 22)]);
            for c in 0..COLS {
                r[5 + c] = 4 + (row as usize * COLS + c) as u8 % 100;
            }
            if row == 2 {
                r[5 + 2] = 0x1A; // W at position 46
            }
            let (_, out) = p.input(1, &r);
            if row == 5 {
                assert_eq!((out[0].data[0], out[0].data[5]), (0x23, 1)); // raw rows on
            }
        }
        assert!(!p.event_driven());
        let row = |n: u8, w: u16| {
            let mut d = reply(&[(1, 0x23), (4, n), (5, 44)]);
            for c in 0..COLS {
                let v: u16 = if n == 3 && c == 2 { w } else { 2400 };
                d[5 + c * 2..7 + c * 2].copy_from_slice(&v.to_be_bytes());
            }
            d
        };
        // released at 2400 (bottom seeded at 1353 for W): nothing until all rows are in
        for n in 1..=5 {
            assert!(p.input(1, &row(n, 2400)).0.is_none());
        }
        assert!(p.input(1, &row(6, 2400)).0.unwrap().is_empty());
        // W pressed to the middle of 2400..1353, three readings for the median
        let mid = (2400 + 1353) / 2;
        let mut keys = None;
        for _ in 0..3 {
            keys = p.input(1, &row(3, mid)).0;
        }
        assert!((keys.unwrap()[&0x1A] - 0.5).abs() < 0.01);
        assert_eq!(p.stop()[0].data[..2], [0x23, 0]);
        // a row going quiet restarts the session
        p.tick(10.0);
        p.tick(10.5);
        assert_eq!(p.step, Step::Identity);
    }

    #[test]
    fn ajazz_other_firmware_is_not_admitted() {
        let mut p = AulaW669::default();
        p.start();
        p.input(1, &m484_info("SG8994HERGB", "V1.14.0"));
        assert!(!p.ak820 && !p.na87);
        assert!(p.problem().unwrap().contains("V1.14.0"));
    }

    #[test]
    fn unknown_firmware_fails_with_a_reason() {
        let mut p = AulaW669::default();
        p.start();
        p.input(1, &info("SOMETHINGELSE"));
        p.input(1, &reply(&[(1, ANALOG), (5, 6), (6, 0x04), (7, 0x90), (10, 0x01)]));
        for n in 0..10 {
            p.input(1, &fragment(n, &[]));
        }
        assert!(p.problem().unwrap().contains("SOMETHINGELSE"));
    }

    #[test]
    fn setup_retries_then_gives_up() {
        let mut p = AulaW669::default();
        p.start();
        let mut t = 0.0;
        for _ in 0..200 {
            t += 0.1;
            p.tick(t);
        }
        assert!(p.problem().is_some());
    }
}

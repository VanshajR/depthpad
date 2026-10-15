//! RongYuan RY5088 keyboards (EPOMAKER, MonsGeek, Akko, EWEADN, AJAZZ, FL ESPORTS, Attack Shark and
//! many more OEM brands), from HallJoy's `rongyuan_stream_protocol.h`, `rongyuan_snapshot_protocol.h`,
//! `rongyuan_stream_backend.cpp`, `attackshark_pro_native_model.h` and `attackshark_pro_diagnostic.cpp`.
//!
//! Requests go as 64-byte feature reports (ID 0) to the control collection (usage 2), with a gap
//! before every transfer as the makers' drivers do. 0x8F gives the board number and firmware
//! version, 0x80 the radio firmware version (it sets the travel units even over USB). Then one
//! of two ways to read depth, by board:
//! - stream: 0xE6 gives the travel units, 0x1B 1 starts per-key events (32-byte input reports,
//!   ID 5, from the input collection FFFF:0001), 0x1B 0 stops them;
//! - Attack Shark: 0xE5 polls a page of 32 key travels at a time (no stream on these).

use super::{le16, Matrix};
use crate::protocols::analog::{AnalogProtocol, Keys, Out};
use crate::protocols::halljoy::rongyuan_tables::{ALIASES, BOARDS, SHARK};

pub struct Board {
    pub board: u32,
    pub vid: u16,
    pub pid: u16,
    pub product: &'static str,
    pub range_um: u32,
    /// Travel units must come from the keyboard (0xE6), not its firmware version.
    pub precision_enum: bool,
    /// (matrix slot, analog code)
    pub keys: &'static [(u8, u16)],
}

fn code(keys: &[(u8, u16)], slot: u8) -> u16 {
    keys.iter().find(|k| k.0 == slot).map_or(0, |k| k.1)
}

/// A streaming board, under the USB IDs it was found with.
pub fn find(board: u32, vid: u16, pid: u16) -> Option<&'static Board> {
    let (vid, pid, board) = ALIASES.iter().find(|a| a.0 == (board, vid, pid)).map_or((vid, pid, board), |a| a.1);
    BOARDS.iter().find(|b| b.board == board && b.vid == vid && b.pid == pid)
}

/// An Attack Shark board: (board, pid, Fn slot, keys). HallJoy admits the R68 HE (3650) on 5029
/// as well, as a tester's keyboard reported.
pub fn find_shark(board: u32, vid: u16, pid: u16) -> Option<&'static (u32, u16, u8, &'static [(u8, u16)])> {
    SHARK.iter().find(|s| vid == 0x3151 && s.0 == board && (s.1 == pid || (board == 3650 && pid == 0x5029)))
}

/// The firmware name an Attack Shark board is matched by (they have no product string here).
pub fn shark_identity(board: u32) -> String {
    format!("SHARK-{board}")
}

const GAP_MS: u32 = 10;
/// Attack Shark polling: the vendor's 1 ms send and read delays.
const POLL_GAP_MS: u32 = 1;
/// Attack Shark travel: a fixed 3.5 mm endpoint.
const SHARK_TRAVEL_UM: u32 = 3500;
/// Reads of one reply before giving up (HallJoy: 100 ms of 10 ms reads).
const TRIES: u32 = 10;
const ATTEMPTS: u32 = 3;

/// A request: command, two arguments and a page, checksummed.
pub fn request(cmd: u8, a: u8, b: u8, page: u8) -> Vec<u8> {
    let mut d = vec![0u8; 64];
    (d[0], d[1], d[2], d[3]) = (cmd, a, b, page);
    let sum: u32 = d[..7].iter().map(|&v| v as u32).sum();
    d[7] = (255 - sum % 256) as u8;
    if cmd == 0xE5 || cmd == 0x8A {
        d[8..].fill(255);
    }
    d
}

/// Attack Shark's page request (its client sends no 0xFF tail).
pub fn shark_page_request(page: u8) -> Vec<u8> {
    let mut d = vec![0u8; 64];
    (d[0], d[1], d[2], d[3]) = (0xE5, 0xFE, 1, page);
    let sum: u32 = d[..7].iter().map(|&v| v as u32).sum();
    d[7] = (255 - sum % 256) as u8;
    d
}

/// Travel units per mm from the firmware version.
fn version_units(version: u16) -> u32 {
    if version >= 0x500 { 200 } else if version >= 0x300 { 100 } else { 10 }
}

/// Travel units per mm: from the 0xE6 reply when it has its marker, else from the version.
pub fn units(version: u16, features: Option<&[u8]>, required: bool) -> u32 {
    if let Some(f) = features.filter(|f| f.len() > 2 && f[0] == 0xE6 && f[1] == 0xAA) {
        return [100, 200, 1000].get(f[2] as usize).copied().unwrap_or(0);
    }
    if required || version == 0 || version == 0xFFFF {
        return 0;
    }
    version_units(version)
}

/// Which 32-slot page to poll next (HallJoy's schedule: mostly page 0, where WASD is).
pub fn shark_page(cycle: u32, fn_page: u8, fourth: bool) -> u8 {
    let other = if fn_page == 1 { 2 } else { 1 };
    if fourth {
        return [fn_page, 0, other, 3][(cycle % 4) as usize];
    }
    if cycle % 128 == 0 {
        return 3;
    }
    [fn_page, 0, 0, 0, 0, fn_page, other, other][((cycle - 1) % 8) as usize]
}

/// A polled page: 32 travels, or None for a reply that isn't one (an echo, or implausible).
pub fn decode_page(d: &[u8]) -> Option<[u16; 32]> {
    if d.len() < 64 || (d[0] == 0xE5 && d[1] == 0xFE && d[2] == 1) || d[0] == 0x8F {
        return None;
    }
    let mut v = [0u16; 32];
    for (i, x) in v.iter_mut().enumerate() {
        *x = le16(d, 2 * i);
        if *x > 4096 {
            return None;
        }
    }
    Some(v)
}

#[derive(Clone, Copy)]
enum Mode {
    Stream(&'static Board),
    Shark { board: u32, fn_page: u8, fourth: bool, keys: &'static [(u8, u16)] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Board,
    Radio,
    Units,
    Live,
    Failed,
}

pub struct RongYuan {
    vid: u16,
    pid: u16,
    step: Step,
    tries: u32,
    attempts: u32,
    board_number: u32,
    mode: Option<Mode>,
    version: u16,
    units: u32,
    matrix: Matrix,
    // Attack Shark polling
    cycle: u32,
    page: Option<u8>,
    discard: u32,
}

impl RongYuan {
    pub fn new(vid: u16, pid: u16) -> Self {
        Self {
            vid, pid, step: Step::Board, tries: 0, attempts: 0, board_number: 0, mode: None, version: 0, units: 0,
            matrix: Matrix::default(), cycle: 0, page: None, discard: 0,
        }
    }

    fn ask(&mut self, step: Step, cmd: u8) -> Vec<Out> {
        self.step = step;
        self.tries = 0;
        vec![Out::pause(GAP_MS), Out::feature(0, 64, &request(cmd, 0, 0, 0)), Out::pause(GAP_MS), Out::get_feature(0, 64)]
    }

    fn read_again(&mut self) -> Vec<Out> {
        vec![Out::pause(GAP_MS), Out::get_feature(0, 64)]
    }

    fn after_radio(&mut self) -> Vec<Out> {
        match self.mode {
            Some(Mode::Stream(_)) => self.ask(Step::Units, 0xE6),
            _ => {
                self.units = version_units(self.version);
                self.go_live(Vec::new())
            }
        }
    }

    fn after_units(&mut self, features: Option<&[u8]>) -> Vec<Out> {
        let Some(Mode::Stream(board)) = self.mode else { return Vec::new() };
        self.units = units(self.version, features, board.precision_enum);
        if self.units == 0 {
            self.step = Step::Failed;
            return Vec::new();
        }
        self.go_live(vec![Out::pause(GAP_MS), Out::feature(0, 64, &request(0x1B, 1, 0, 0))])
    }

    fn go_live(&mut self, outs: Vec<Out>) -> Vec<Out> {
        self.step = Step::Live;
        self.attempts = 0;
        outs
    }

    fn shark(&self) -> Option<(u8, bool, &'static [(u8, u16)])> {
        match self.mode {
            Some(Mode::Shark { fn_page, fourth, keys, .. }) => Some((fn_page, fourth, keys)),
            _ => None,
        }
    }
}

impl AnalogProtocol for RongYuan {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            self.step = Step::Failed;
            return Vec::new();
        }
        self.ask(Step::Board, 0x8F)
    }

    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        // key travel: 1B, raw travel (LE), slot; other ID 5 reports are notifications
        let Some(Mode::Stream(board)) = self.mode else { return (None, Vec::new()) };
        if self.step != Step::Live || id != 5 || d.len() < 31 || d[0] != 0x1B || d[3] >= 128 {
            return (None, Vec::new());
        }
        let raw = le16(d, 1) as u32;
        if raw > 6 * self.units {
            return (None, Vec::new());
        }
        let depth = raw as f64 * 1000.0 / (self.units as f64 * board.range_um as f64);
        self.matrix.set(d[3] as usize, code(board.keys, d[3]), depth);
        (Some(self.matrix.keys()), Vec::new())
    }

    fn feature(&mut self, _id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        let retry_or = |s: &mut Self, give_up: fn(&mut Self) -> Vec<Out>| {
            s.tries += 1;
            if s.tries < TRIES { s.read_again() } else { give_up(s) }
        };
        let outs = match self.step {
            Step::Board if d.len() >= 9 && d[0] == 0x8F => {
                self.board_number = u32::from_le_bytes([d[1], d[2], d[3], d[4]]);
                self.version = le16(d, 7);
                self.mode = find(self.board_number, self.vid, self.pid).map(Mode::Stream).or_else(|| {
                    find_shark(self.board_number, self.vid, self.pid).map(|s| Mode::Shark {
                        board: s.0,
                        fn_page: s.2 / 32,
                        fourth: s.3.iter().any(|k| k.0 >= 96),
                        keys: s.3,
                    })
                });
                if self.mode.is_none() {
                    self.step = Step::Failed;
                    Vec::new()
                } else {
                    self.ask(Step::Radio, 0x80)
                }
            }
            // the reply isn't built yet: read again, then start over
            Step::Board => retry_or(self, |s| s.start()),
            Step::Radio if d.len() >= 3 && d[0] == 0x80 => {
                let radio = le16(d, 1);
                if radio != 0 && radio != 0xFFFF {
                    self.version = radio;
                }
                self.after_radio()
            }
            Step::Radio => retry_or(self, |s| s.after_radio()),
            Step::Units if d.len() >= 3 && d[0] == 0xE6 && d[1] == 0xAA => self.after_units(Some(d)),
            Step::Units => retry_or(self, |s| s.after_units(None)),
            Step::Live => {
                if let (Some((_, _, keys)), Some(page)) = (self.shark(), self.page) {
                    if self.discard > 0 {
                        self.discard -= 1;
                    } else if let Some(v) = decode_page(d) {
                        for (i, raw) in v.iter().enumerate() {
                            let slot = page as usize * 32 + i;
                            let depth = *raw as f64 * 1000.0 / (self.units as f64 * SHARK_TRAVEL_UM as f64);
                            self.matrix.set(slot, code(keys, slot as u8), depth);
                        }
                        return (Some(self.matrix.keys()), Vec::new());
                    }
                }
                Vec::new()
            }
            Step::Failed => Vec::new(),
        };
        let ready = self.step == Step::Live && self.shark().is_none();
        (if ready { Some(Keys::new()) } else { None }, outs)
    }

    fn tick(&mut self, _now: f64) -> Vec<Out> {
        let Some((fn_page, fourth, _)) = self.shark().filter(|_| self.step == Step::Live) else { return Vec::new() };
        self.cycle = self.cycle.wrapping_add(1);
        let page = shark_page(self.cycle, fn_page, fourth);
        // replies carry no page number: after switching pages, the first one may be the old page
        let rounds = if self.page != Some(page) { 2 } else { 1 };
        self.discard = rounds - 1;
        self.page = Some(page);
        let ask = [Out::pause(POLL_GAP_MS), Out::feature(0, 64, &shark_page_request(page)), Out::pause(POLL_GAP_MS), Out::get_feature(0, 64)];
        ask.iter().cloned().cycle().take(4 * rounds as usize).collect()
    }

    fn identity(&self) -> Option<String> {
        match self.mode? {
            Mode::Stream(b) => Some(b.product.to_string()),
            Mode::Shark { board, .. } => Some(shark_identity(board)),
        }
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| match self.mode {
            None if self.board_number != 0 => format!(
                "board {} on {:04X}:{:04X} isn't one Depthpad knows the key matrix of yet",
                self.board_number, self.vid, self.pid
            ),
            Some(_) => "the keyboard didn't report its travel units".to_string(),
            None => "the keyboard didn't complete analog setup".to_string(),
        })
    }

    fn input_collection(&self) -> Option<(u16, u16)> {
        Some((0xFFFF, 1))
    }

    fn read_timeout_ms(&self) -> Option<u32> {
        match self.mode {
            Some(Mode::Stream(_)) if self.step == Step::Live => Some(5),
            _ => None,
        }
    }

    fn event_driven(&self) -> bool {
        self.step == Step::Live && matches!(self.mode, Some(Mode::Stream(_)))
    }

    fn stop(&mut self) -> Vec<Out> {
        if self.step == Step::Live && matches!(self.mode, Some(Mode::Stream(_))) {
            vec![Out::pause(GAP_MS), Out::feature(0, 64, &request(0x1B, 0, 0, 0))]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::analog::OutKind;

    fn reply(head: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; 64];
        d[..head.len()].copy_from_slice(head);
        d
    }

    fn board_reply(n: u32, version: u16) -> Vec<u8> {
        let b = n.to_le_bytes();
        reply(&[0x8F, b[0], b[1], b[2], b[3], 0, 0, version as u8, (version >> 8) as u8])
    }

    #[test]
    fn requests_are_checksummed() {
        let r = request(0x1B, 1, 0, 0);
        assert_eq!((r[0], r[1], r[7]), (0x1B, 1, 255 - 0x1C));
        assert!(request(0x8A, 0, 255, 2)[8..].iter().all(|&b| b == 255));
        // HallJoy's own checks: Request(0xe5,0)[8]==0x1b, Request(0xe5,3)[8]==0x18
        assert_eq!(shark_page_request(0)[7], 0x1B);
        assert_eq!(shark_page_request(3)[7], 0x18);
    }

    #[test]
    fn units_from_reply_or_version() {
        assert_eq!(units(0x0310, Some(&reply(&[0xE6, 0xAA, 2])), false), 1000);
        assert_eq!(units(0x0510, None, false), 200);
        assert_eq!(units(0x0310, None, false), 100);
        assert_eq!(units(0x0510, None, true), 0);
    }

    #[test]
    fn epomaker_he108_session() {
        // EPOMAKER HE108: board 3365 on 3151:5030 (HallJoy's confirmed device)
        let b = BOARDS.iter().find(|b| b.board == 3365).unwrap();
        assert_eq!((b.vid, b.pid), (0x3151, 0x5030));
        let mut p = RongYuan::new(0x3151, 0x5030);
        let outs = p.start();
        assert_eq!(outs.iter().map(|o| o.kind).collect::<Vec<_>>(),
                   [OutKind::Pause(10), OutKind::Feature, OutKind::Pause(10), OutKind::GetFeature(64)]);
        assert_eq!(outs[1].data[0], 0x8F);
        assert_eq!(p.read_timeout_ms(), None);
        // not ready yet: read again
        assert_eq!(p.feature(0, &reply(&[0])).1.len(), 2);
        let (_, outs) = p.feature(0, &board_reply(3365, 0x0510));
        assert_eq!(outs[1].data[0], 0x80);
        assert_eq!(p.identity().as_deref(), Some(b.product));
        let (_, outs) = p.feature(0, &reply(&[0x80, 0xFF, 0xFF]));
        assert_eq!(outs[1].data[0], 0xE6);
        let (keys, outs) = p.feature(0, &reply(&[0xE6, 0xAA, 1])); // 200 units/mm
        assert_eq!(keys, Some(Keys::new()));
        assert_eq!((outs[1].data[0], outs[1].data[1]), (0x1B, 1));
        assert!(p.event_driven());
        assert_eq!(p.read_timeout_ms(), Some(5));
        // W half way down
        let (slot, _) = *b.keys.iter().find(|k| k.1 == 0x1A).unwrap();
        let raw = (b.range_um / 2 / 5) as u16; // range/2 in units of 5 µm (200 per mm)
        let mut ev = vec![0u8; 31];
        (ev[0], ev[1], ev[2], ev[3]) = (0x1B, raw as u8, (raw >> 8) as u8, slot);
        let (keys, _) = p.input(5, &ev);
        assert!((keys.unwrap()[&0x1A] - 0.5).abs() < 0.01);
        assert_eq!(p.stop()[1].data[..2], [0x1B, 0]);
    }

    #[test]
    fn attack_shark_polls_pages() {
        // ATTACK SHARK X65 Pro HE: board 2308 on 3151:502F, Fn at slot 65 (page 2)
        let mut p = RongYuan::new(0x3151, 0x502F);
        p.start();
        p.feature(0, &board_reply(2308, 0x0310));
        assert_eq!(p.identity().as_deref(), Some("SHARK-2308"));
        let (keys, outs) = p.feature(0, &reply(&[0x80, 0, 0]));
        assert!(outs.is_empty() && keys.is_none());
        assert!(!p.event_driven());
        assert_eq!(p.units, 100);
        // first poll: page 2 (the Fn page), flushed: two exchanges, the first reply dropped
        let outs = p.tick(0.0);
        assert_eq!(outs.len(), 8);
        assert_eq!(outs[1].data[..4], [0xE5, 0xFE, 1, 2]);
        assert!(p.feature(0, &reply(&[])).0.is_none());
        assert!(p.feature(0, &reply(&[])).0.is_some());
        // next: page 0, where W is (slot 14): 175 units of 10 µm = 1.75 mm of 3.5
        let outs = p.tick(0.0);
        assert_eq!(outs[1].data[3], 0);
        p.feature(0, &reply(&[]));
        let mut page = vec![0u8; 64];
        page[28] = 175;
        let keys = p.feature(0, &page).0.unwrap();
        assert!((keys[&0x1A] - 0.5).abs() < 1e-9);
        // the same page again: no flush
        assert_eq!(p.tick(0.0).len(), 4);
        // an echoed request isn't a page
        assert!(p.feature(0, &reply(&[0xE5, 0xFE, 1])).0.is_none());
    }

    #[test]
    fn unknown_board_is_a_problem() {
        let mut p = RongYuan::new(0x3151, 0x5030);
        p.start();
        p.feature(0, &board_reply(1, 0));
        assert!(p.problem().unwrap().contains("board 1 on 3151:5030"));
    }

    #[test]
    fn aliases_resolve() {
        // FUN60 Pro sold as 3151:502D with board 2304 uses board 2600's entry
        assert_eq!(find(2304, 0x3151, 0x502D).unwrap().board, 2600);
        assert!(find_shark(3650, 0x3151, 0x5029).is_some());
    }

    #[test]
    fn page_schedule() {
        // compact boards: mostly page 0, page 3 every 128 polls
        let pages: Vec<u8> = (1..=8).map(|c| shark_page(c, 2, false)).collect();
        assert_eq!(pages, [2, 0, 0, 0, 0, 2, 1, 1]);
        assert_eq!(shark_page(128, 2, false), 3);
        assert_eq!((0..4).map(|c| shark_page(c, 2, true)).collect::<Vec<_>>(), [2, 0, 1, 3]);
    }
}

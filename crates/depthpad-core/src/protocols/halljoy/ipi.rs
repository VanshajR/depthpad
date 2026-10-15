//! IPI (QBZ75, Aurora 75 / 75 PRO, QBZ65 / Aurora65 / Rain65, flash68) and Royal Kludge RK68 HE
//! keyboards (BY firmware, addressed analog protocol), from HallJoy's `ipi_protocol.h`,
//! `generated/ipi_models.h` and `addressed_analog_backend.cpp`.
//!
//! Collection FF60:0061, 64-byte reports with ID 9; each frame ends in a checksum that makes the
//! byte sum (ID included) 0xFF. 82/01 gives the model's UUID, which says which key IDs it scans;
//! 94/05 gives each key's calibration (raw value released and bottomed out), and 94/02 its raw
//! value now, nine keys per request, polled round the keyboard.

use super::Matrix;
use crate::protocols::analog::{AnalogProtocol, Keys, Out};
use crate::protocols::halljoy::ipi_tables::{FACTORY, MODELS};

const ID: u8 = 9;
const LEN: usize = 63;
const BATCH: usize = 9;
/// HallJoy waits 8 ms for a reply; the driver ticks about every 5 ms.
const REPLY_SECS: f64 = 0.03;
const FAILURES: u32 = 20;
const ATTEMPTS: u32 = 3;

fn finish(mut d: Vec<u8>) -> Out {
    let sum: u32 = ID as u32 + d[..LEN - 1].iter().map(|&b| b as u32).sum::<u32>();
    d[LEN - 1] = (0xFF - sum % 256) as u8;
    Out { report_id: ID, data: d, kind: crate::protocols::analog::OutKind::Write }
}

pub fn uuid_request() -> Out {
    let mut d = vec![0u8; LEN];
    (d[0], d[1], d[3], d[5]) = (0x82, 1, 1, 6);
    finish(d)
}

/// A request for some keys (cmd 94, sub 05 calibration or 02 values).
pub fn key_request(cmd: u8, sub: u8, ids: &[u8]) -> Out {
    let mut d = vec![0u8; LEN];
    (d[0], d[1], d[3], d[5]) = (cmd, sub, 1, (ids.len() * 2) as u8);
    for (i, &id) in ids.iter().enumerate() {
        d[7 + i * 2] = id;
    }
    finish(d)
}

/// A reply's records (key ID, two values), if it answers cmd/sub for exactly these keys.
fn records(d: &[u8], cmd: u8, sub: u8, ids: &[u8]) -> Option<Vec<(u8, u16, u16)>> {
    let sum: u32 = ID as u32 + d.iter().take(LEN).map(|&b| b as u32).sum::<u32>();
    let len = ids.len() * 6;
    if d.len() < LEN || sum % 256 != 255 || d[0] != cmd || d[1] != sub || d[2] != 0 || d[3] != 1 || d[4] != 0 || d[5] as usize != len {
        return None;
    }
    let be = |i: usize| u16::from_be_bytes([d[i], d[i + 1]]);
    let mut out = Vec::with_capacity(ids.len());
    for i in 0..ids.len() {
        let id = be(6 + i * 6);
        if id == 0 || id > 255 || !ids.contains(&(id as u8)) || out.iter().any(|r: &(u8, u16, u16)| r.0 == id as u8) {
            return None;
        }
        out.push((id as u8, be(8 + i * 6), be(10 + i * 6)));
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Uuid,
    Calibrate(usize),
    Poll(usize),
    Failed,
}

pub struct Ipi {
    step: Step,
    model: Option<&'static (u64, &'static str, &'static [u8])>,
    uuid: u64,
    /// Raw value released, bottomed out, per key ID.
    calibration: [(u16, u16); 256],
    sent_at: Option<f64>,
    failures: u32,
    attempts: u32,
    matrix: Matrix,
}

impl Default for Ipi {
    fn default() -> Self {
        Self { step: Step::Uuid, model: None, uuid: 0, calibration: [(0, 0); 256], sent_at: None, failures: 0, attempts: 0, matrix: Matrix::default() }
    }
}

impl Ipi {
    fn batch(&self, n: usize) -> &'static [u8] {
        let ids = self.model.map_or(&[][..], |m| m.2);
        &ids[(n * BATCH).min(ids.len())..((n + 1) * BATCH).min(ids.len())]
    }

    fn batches(&self) -> usize {
        self.model.map_or(0, |m| m.2.len().div_ceil(BATCH))
    }

    fn ask(&mut self, step: Step) -> Vec<Out> {
        self.step = step;
        self.sent_at = None;
        match step {
            Step::Uuid => vec![uuid_request()],
            Step::Calibrate(n) => vec![key_request(0x94, 5, self.batch(n))],
            Step::Poll(n) => vec![key_request(0x94, 2, self.batch(n))],
            Step::Failed => Vec::new(),
        }
    }
}

/// Depth from a raw value: the sensor reads lower as the key goes down.
pub fn depth(raw: u16, (released, bottom): (u16, u16)) -> f64 {
    if raw == 0 || raw >= released || released <= bottom {
        return 0.0;
    }
    if raw <= bottom {
        return 1.0;
    }
    let d = (released - raw) as f64 / (released - bottom) as f64;
    if d < 0.008 { 0.0 } else { d }
}

impl AnalogProtocol for Ipi {
    fn start(&mut self) -> Vec<Out> {
        self.matrix.clear();
        self.failures = 0;
        self.attempts += 1;
        if self.attempts > ATTEMPTS {
            self.step = Step::Failed;
            return Vec::new();
        }
        self.ask(Step::Uuid)
    }

    fn input(&mut self, id: u8, d: &[u8]) -> (Option<Keys>, Vec<Out>) {
        if id != ID {
            return (None, Vec::new());
        }
        match self.step {
            Step::Uuid => {
                if let Some(u) = records_uuid(d) {
                    self.uuid = u;
                    self.model = MODELS.iter().find(|m| m.0 == u);
                    if self.model.is_none() {
                        self.step = Step::Failed;
                        return (None, Vec::new());
                    }
                    return (None, self.ask(Step::Calibrate(0)));
                }
            }
            Step::Calibrate(n) => {
                if let Some(recs) = records(d, 0x94, 5, self.batch(n)) {
                    // released must sit well above bottomed out
                    if recs.iter().any(|&(_, rel, bot)| bot == 0 || rel > 0x7FFF || rel as u32 <= bot as u32 + 256) {
                        self.step = Step::Failed;
                        return (None, Vec::new());
                    }
                    for (k, rel, bot) in recs {
                        self.calibration[k as usize] = (rel, bot);
                    }
                    self.failures = 0;
                    let next = if n + 1 < self.batches() { Step::Calibrate(n + 1) } else { Step::Poll(0) };
                    if next == Step::Poll(0) {
                        self.attempts = 0;
                    }
                    return (None, self.ask(next));
                }
            }
            Step::Poll(n) => {
                if let Some(recs) = records(d, 0x94, 2, self.batch(n)) {
                    for (k, value, _) in recs {
                        let raw = value & 0x7FFF;
                        let code = FACTORY.get(k as usize).copied().unwrap_or(0);
                        self.matrix.set(k as usize, code, depth(raw, self.calibration[k as usize]));
                    }
                    self.failures = 0;
                    let last = n + 1 >= self.batches();
                    let outs = self.ask(Step::Poll(if last { 0 } else { n + 1 }));
                    return (last.then(|| self.matrix.keys()), outs);
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
        if self.failures >= FAILURES {
            return self.start();
        }
        self.ask(self.step)
    }

    fn identity(&self) -> Option<String> {
        self.model.map(|m| m.1.to_string())
    }

    fn problem(&self) -> Option<String> {
        (self.step == Step::Failed).then(|| match self.model {
            None if self.uuid != 0 => format!("model {:012X} isn't one Depthpad knows the keys of yet", self.uuid),
            Some(_) => "the keyboard reported unusable calibration".to_string(),
            None => "the keyboard didn't answer analog requests".to_string(),
        })
    }
}

/// The model UUID from an 82/01 reply.
fn records_uuid(d: &[u8]) -> Option<u64> {
    let sum: u32 = ID as u32 + d.iter().take(LEN).map(|&b| b as u32).sum::<u32>();
    if d.len() < LEN || sum % 256 != 255 || d[0] != 0x82 || d[1] != 1 || d[2] != 0 || d[3] != 1 || d[4] != 0 || d[5] != 6 {
        return None;
    }
    Some(d[6..12].iter().fold(0u64, |v, &b| v << 8 | b as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reply frame (without the ID), checksummed.
    fn reply(cmd: u8, sub: u8, body: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; LEN];
        (d[0], d[1], d[3], d[5]) = (cmd, sub, 1, body.len() as u8);
        d[6..6 + body.len()].copy_from_slice(body);
        let sum: u32 = ID as u32 + d[..LEN - 1].iter().map(|&b| b as u32).sum::<u32>();
        d[LEN - 1] = (0xFF - sum % 256) as u8;
        d
    }

    fn recs(ids: &[u8], a: u16, b: u16) -> Vec<u8> {
        ids.iter().flat_map(|&i| [0, i, (a >> 8) as u8, a as u8, (b >> 8) as u8, b as u8]).collect()
    }

    #[test]
    fn requests_sum_to_ff() {
        for o in [uuid_request(), key_request(0x94, 2, &[1, 2, 3])] {
            let sum: u32 = ID as u32 + o.data.iter().map(|&b| b as u32).sum::<u32>();
            assert_eq!(sum % 256, 255);
        }
    }

    #[test]
    fn rk68_he_session() {
        let mut p = Ipi::default();
        assert_eq!(p.start()[0].data[..2], [0x82, 1]);
        let (_, o) = p.input(ID, &reply(0x82, 1, &[0x11, 0, 0, 0, 0, 0x02]));
        assert_eq!(p.identity().as_deref(), Some("110000000002"));
        assert_eq!(o[0].data[..2], [0x94, 5]);
        let model = p.model.unwrap();
        // calibrate every batch: released 3000, bottom 1000
        let mut n = 0;
        while let Step::Calibrate(b) = p.step {
            let ids = p.batch(b);
            p.input(ID, &reply(0x94, 5, &recs(ids, 3000, 1000)));
            n += 1;
        }
        assert_eq!(n, model.2.len().div_ceil(BATCH));
        // W: find its key ID, press it half way (raw 2000) in its batch
        let w = model.2.iter().copied().find(|&k| FACTORY[k as usize] == 0x1A).unwrap();
        let mut keys = None;
        for b in 0..p.batches() {
            assert_eq!(p.step, Step::Poll(b));
            let ids = p.batch(b);
            let body: Vec<u8> = ids.iter().flat_map(|&i| {
                let v: u16 = if i == w { 2000 } else { 3000 };
                [0, i, (v >> 8) as u8, v as u8, 0, 0]
            }).collect();
            keys = p.input(ID, &reply(0x94, 2, &body)).0.or(keys);
        }
        assert_eq!(keys.unwrap()[&0x1A], 0.5);
    }

    #[test]
    fn depth_curve() {
        assert_eq!(depth(3000, (3000, 1000)), 0.0);
        assert_eq!(depth(900, (3000, 1000)), 1.0);
        assert_eq!(depth(2990, (3000, 1000)), 0.0); // under 0.8%
    }

    #[test]
    fn unknown_uuid() {
        let mut p = Ipi::default();
        p.start();
        p.input(ID, &reply(0x82, 1, &[0x99, 0, 0, 0, 0, 1]));
        assert!(p.problem().unwrap().contains("990000000001"));
    }
}

//! Sonix-based AJAZZ keyboards (AK680 V2 HE, 0C45:80BC) vendor protocol. Pure parsing and
//! building, no I/O; the driver is `platform::drivers::sonix_ajazz`.
//!
//! Requests are 64-byte reports (plus report ID 0) on the vendor collection FF68:0061:
//!   `AA <cmd> <len> <addr lo> <addr hi> 00 <last> 00 <data...>`
//! Replies echo `55 <cmd> <len> <addr lo> <addr hi>`. With SET_SIMULATION_TEST_ON (0x66) the
//! keyboard streams `55 FB` packets at ~1 kHz:
//!   [2] position, [3] calibration status, [4..5] max ADC, [6..7] min ADC, [8..9] ADC,
//!   [10..11] travel (0.01 mm), [12..13] full stroke (0.1 mm).
//! Measured on fw 11.01: the stream only follows the most recently moved key (docs/PROTOCOL.md).

use std::collections::{BTreeMap, HashMap};

use super::Sample;
use crate::keymap::Keymap;

pub const REPORT_BYTES: usize = 65;

pub const GET_DEVICE_INFO: u8 = 0x10;
pub const GET_GAME_MODE: u8 = 0x11;
pub const GET_KEY: u8 = 0x12;
pub const SIMULATION_ON: u8 = 0x66;
pub const SIMULATION_OFF: u8 = 0x67;
pub const STREAM: u8 = 0xFB;
/// Read the current state of up to 7 named keys (Driveall's getMagneticAxisStatus): the request
/// carries 8 bytes per key, the key position first; the reply carries 8 bytes per key:
/// calibration status, ADC (u16 LE), current travel (u16 LE, 0.01 mm).
pub const GET_KEY_STATUS: u8 = 0x68;
/// Per-key actuation and Rapid Trigger settings (Driveall's getMagneticAxisRT): 128 records of
/// 8 bytes: switch type, flags, actuation travel (u16 LE), press and release sensitivity.
pub const GET_ACTUATION: u8 = 0x17;
pub const ACTUATION_BYTES: u16 = 1024;

/// Each key's actuation point in 0.01 mm, from GET_ACTUATION data; implausible values dropped.
pub fn parse_actuation(data: &[u8], rt_precision: u8) -> BTreeMap<u8, f64> {
    let per_mm = if rt_precision == 2 { 1000.0 } else { 100.0 };
    data.chunks_exact(8)
        .take(128)
        .enumerate()
        .filter_map(|(pos, r)| {
            let mm = u16::from_le_bytes([r[2], r[3]]) as f64 / per_mm;
            (0.05..=5.0).contains(&mm).then_some((pos as u8, mm * 100.0))
        })
        .collect()
}

/// One output report, report ID first.
pub fn request(cmd: u8, len: u8, addr: u16, last: bool, data: &[u8]) -> [u8; REPORT_BYTES] {
    let mut r = [0u8; REPORT_BYTES];
    r[1] = 0xAA;
    r[2] = cmd;
    r[3] = len;
    r[4] = (addr & 0xFF) as u8;
    r[5] = (addr >> 8) as u8;
    r[7] = last as u8;
    let n = data.len().min(REPORT_BYTES - 9);
    r[9..9 + n].copy_from_slice(&data[..n]);
    r
}

/// A reply to `request(cmd, len, addr)`: `55 <cmd> <len> <addr>` (as HallJoy's Reply()).
/// `report` excludes the report ID.
pub fn is_reply(report: &[u8], cmd: u8, len: u8, addr: u16) -> bool {
    report.len() >= 8
        && report[0] == 0x55
        && report[1] == cmd
        && report[2] == len
        && u16::from_le_bytes([report[3], report[4]]) == addr
}

/// One key's state from a GET_KEY_STATUS reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStatus {
    pub position: u8,
    pub calibration: u8,
    pub adc: u16,
    pub travel: u16,
}

/// The request data for GET_KEY_STATUS: 8 bytes per key, position first (at most 7 keys).
pub fn key_status_data(keys: &[u8]) -> Vec<u8> {
    keys.iter().take(7).flat_map(|&k| [k, 0, 0, 0, 0, 0, 0, 0]).collect()
}

/// Parse a GET_KEY_STATUS reply (report without its ID) for the keys that were asked for.
pub fn parse_key_status(report: &[u8], keys: &[u8]) -> Vec<KeyStatus> {
    let data = report.get(8..).unwrap_or(&[]);
    keys.iter()
        .take(7)
        .enumerate()
        .filter_map(|(i, &k)| {
            let d = data.get(i * 8..i * 8 + 5)?;
            Some(KeyStatus {
                position: k,
                calibration: d[0],
                adc: u16::from_le_bytes([d[1], d[2]]),
                travel: u16::from_le_bytes([d[3], d[4]]),
            })
        })
        .collect()
}

/// A stream packet, or None for anything else.
///
/// About 1.5% of packets are sent while the firmware is clearing its buffer. The clearing
/// reaches the header or the stroke field before travel (45 s capture: 653 bad headers,
/// 22 packets with stroke 0 and zeroed or half-zeroed travel, no other damage), so a sane
/// stroke is required.
pub fn parse_stream(report: &[u8]) -> Option<Sample> {
    if report.len() < 14 || report[0] != 0x55 || report[1] != STREAM || report[2] >= 128 {
        return None;
    }
    let travel = u16::from_le_bytes([report[10], report[11]]);
    let stroke = u16::from_le_bytes([report[12], report[13]]);
    if !(10..=50).contains(&stroke) || travel > stroke * 10 + 100 {
        return None;
    }
    Some(Sample { position: report[2], travel, stroke })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vid: u16,
    pub pid: u16,
    pub version: String,
    pub manufacturer: u16,
    pub product: u16,
    /// Units of the actuation travel in GET_ACTUATION records: 2 = 0.001 mm, else 0.01 mm.
    pub rt_precision: u8,
}

/// GET_DEVICE_INFO reply (without report ID).
pub fn parse_device_info(report: &[u8]) -> Option<DeviceInfo> {
    let p = report.get(8..)?;
    if p.len() < 16 {
        return None;
    }
    let u16_at = |o: usize| u16::from_le_bytes([p[o], p[o + 1]]);
    Some(DeviceInfo {
        vid: u16_at(4),
        pid: u16_at(6),
        version: format!("{:x}.{:02x}", p[8], p[9]),
        manufacturer: u16_at(12),
        product: u16_at(14),
        rt_precision: p.get(29).copied().unwrap_or(0),
    })
}

pub fn keyboard_usage(u: u16) -> bool {
    (4..=0xA4).contains(&u) || (0xE0..=0xE7).contains(&u)
}

/// HID usage a physical key currently sends, from its 4-byte GET_KEY record.
///
/// Port of HallJoy's `mini60::Assigned` with the Driveall rules it uses for the AK820
/// (aula_mini60_native_model.h): type 0 is the factory key; Driveall advanced keys (DKS 8,
/// MT 9, TGL 10, SOCD 11, RS 12) keep their own key; type 2 is a keyboard key with p1 one
/// modifier bit or p2 a usage; anything else (macros, media, Fn 0xAF) sends no single usage.
pub fn assigned_usage(factory: u16, record: [u8; 4]) -> Option<u16> {
    let [kind, p1, p2, p3] = record;
    if !keyboard_usage(factory) {
        return None; // e.g. Fn: handled inside the keyboard, never sent to Windows
    }
    if kind == 0 || (8..=12).contains(&kind) {
        return Some(factory);
    }
    if kind != 2 || p3 != 0 {
        return None;
    }
    if p1 != 0 {
        if p2 != 0 || p1.count_ones() != 1 {
            return None;
        }
        return Some(224 + p1.trailing_zeros() as u16);
    }
    keyboard_usage(p2 as u16).then_some(p2 as u16)
}

/// Each physical key -> the usage it sends now (remaps applied), and the reverse.
pub fn key_layout(keymap: &Keymap, records: Option<&[[u8; 4]]>) -> (BTreeMap<u8, u16>, HashMap<u16, u8>) {
    let mut to_usage = BTreeMap::new();
    for (&pos, info) in keymap {
        let rec = records.and_then(|r| r.get(pos as usize)).copied().unwrap_or([0; 4]);
        if let Some(u) = assigned_usage(info.hid, rec) {
            to_usage.insert(pos, u);
        }
    }
    let mut to_pos = HashMap::new();
    for (&pos, &u) in &to_usage {
        to_pos.entry(u).or_insert(pos); // two keys sending one usage: the first owns it
    }
    (to_usage, to_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Keymap;

    fn keymap() -> &'static Keymap {
        &crate::device::model("ajazz-ak680-v2-he").unwrap().keys
    }

    fn pkt(hex: &str) -> Vec<u8> {
        let mut v: Vec<u8> = hex.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
        v.resize(64, 0);
        v
    }

    #[test]
    fn good_packet() {
        let s = parse_stream(&pkt("55 fb 31 01 eb 09 78 05 26 06 61 01 22 00")).unwrap();
        assert_eq!(s, Sample { position: 49, travel: 353, stroke: 34 });
    }

    #[test]
    fn malformed_fixtures_from_captures() {
        for b in [
            "00 00 31 01 eb 09 78 85 28 06 60 01 22 00",     // header cleared
            "00 fb 31 01 eb 09 78 85 27 06 60 01 22 00",     // first byte cleared
            "00 00 00 00 00 00 00 00 26 06 61 01 22 00",     // first 8 bytes cleared
            "55 fb 24 00 00 00 00 00 00 00 00 00 00 00",     // header kept, rest cleared
            "55 fb 22 01 f3 09 78 05 4f 06 5a 00 00 00",     // travel half-cleared, stroke 0
            "55 fb 44 01 fc 09 78 00 00 00 00 00 00 00",     // stroke 0
            "55 fb 90 01 eb 09 78 05 26 06 61 01 22 00",     // position >= 128
            "55 fb 31 01 eb 09 78 05 26 06 ff 7f 22 00",     // travel beyond any stroke
            "55 10 30 00 00 00 01 00 00 00 00 92 45 0c bc 80", // a command reply
        ] {
            assert!(parse_stream(&pkt(b)).is_none(), "{b}");
        }
        assert!(parse_stream(&[0x55, 0xfb, 0x31]).is_none());
    }

    #[test]
    fn request_layout_and_reply_match() {
        let r = request(GET_KEY, 56, 56, false, &[]);
        assert_eq!(&r[..8], &[0, 0xAA, 0x12, 56, 56, 0, 0, 0]);
        let reply = pkt("55 12 38 38 00 00 00 00");
        assert!(is_reply(&reply, GET_KEY, 56, 56));
        assert!(!is_reply(&reply, GET_KEY, 56, 0));
    }

    #[test]
    fn device_info_from_capture() {
        let i = parse_device_info(&pkt(
            "55 10 30 00 00 00 01 00 00 00 00 92 45 0c bc 80 11 01 00 00 66 01 0c 11 0a 10",
        ))
        .unwrap();
        assert_eq!((i.vid, i.pid, i.version.as_str(), i.manufacturer, i.product), (0x0C45, 0x80BC, "11.01", 0x0166, 0x110C));
    }

    #[test]
    fn assigned_usage_rules() {
        let w = 26;
        assert_eq!(assigned_usage(w, [0, 0, 0, 0]), Some(w));
        assert_eq!(assigned_usage(w, [11, 3, 50, 0]), Some(w)); // SOCD keeps its key
        assert_eq!(assigned_usage(w, [2, 0, 4, 0]), Some(4)); // remapped to A
        assert_eq!(assigned_usage(w, [2, 0x02, 0, 0]), Some(225)); // Left Shift
        assert_eq!(assigned_usage(w, [2, 0x03, 0, 0]), None); // two modifiers
        assert_eq!(assigned_usage(w, [2, 0, 0xAF, 0]), None); // Fn
        assert_eq!(assigned_usage(w, [3, 0, 0, 0]), None); // macro
        assert_eq!(assigned_usage(0, [0, 0, 0, 0]), None); // Fn's own key
    }

    #[test]
    fn key_layout_follows_remaps() {
        let (to_usage, to_pos) = key_layout(keymap(), None);
        assert_eq!(to_usage.len(), 67);
        assert_eq!(to_pos[&26], 34);
        let mut rec = vec![[0u8; 4]; 128];
        rec[48] = [2, 0, 86, 0]; // Caps -> keypad minus
        let (to_usage, to_pos) = key_layout(keymap(), Some(&rec));
        assert_eq!(to_usage[&48], 86);
        assert_eq!(to_pos[&86], 48);
        assert!(!to_pos.contains_key(&57));
    }
}

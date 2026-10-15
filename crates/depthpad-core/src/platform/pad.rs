//! The virtual controller, talking to the ViGEmBus driver directly (no client DLL): an Xbox 360
//! controller (XInput, what almost every PC game expects) or a DualShock 4 (for games and
//! tools that want a PlayStation pad). Reports are Xbox-shaped; the DualShock 4 gets them
//! translated (A -> Cross, B -> Circle, X -> Square, Y -> Triangle, Back -> Share,
//! Start -> Options, Guide -> PS, triggers also press L2/R2).

use serde::{Deserialize, Serialize};
use vigem_client::{Client, DS4Report, DualShock4Wired, TargetId, XButtons, XGamepad, Xbox360Wired};

use crate::mapper::Report;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PadKind {
    #[default]
    Xbox,
    Ds4,
}

enum Target {
    Xbox(Xbox360Wired<Client>, XGamepad),
    Ds4(DualShock4Wired<Client>, DS4Report),
}

pub struct Pad {
    target: Target,
}

fn axis(v: f64) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn trigger(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// -1..1 -> 0..255 around 0x80; `invert` for the DualShock's Y axes (up is 0).
fn byte_axis(v: f64, invert: bool) -> u8 {
    let v = if invert { -v } else { v };
    (127.5 + v.clamp(-1.0, 1.0) * 127.5).round().clamp(0.0, 255.0) as u8
}

/// An Xbox-style report as a DualShock 4 report.
pub fn to_ds4(r: &Report) -> DS4Report {
    let b = r.buttons;
    let on = |bit: u16| b & bit != 0;
    let (up, down, left, right) = (on(0x0001), on(0x0002), on(0x0004), on(0x0008));
    let hat: u16 = match (up, right, down, left) {
        (true, false, false, false) => 0,
        (true, true, false, false) => 1,
        (false, true, false, false) => 2,
        (false, true, true, false) => 3,
        (false, false, true, false) => 4,
        (false, false, true, true) => 5,
        (false, false, false, true) => 6,
        (true, false, false, true) => 7,
        _ => 8, // none (or impossible combinations)
    };
    let mut buttons = hat;
    for (x, ds4) in [
        (0x4000, 1 << 4),  // X -> Square
        (0x1000, 1 << 5),  // A -> Cross
        (0x2000, 1 << 6),  // B -> Circle
        (0x8000, 1 << 7),  // Y -> Triangle
        (0x0100, 1 << 8),  // LB -> L1
        (0x0200, 1 << 9),  // RB -> R1
        (0x0020, 1 << 12), // Back -> Share
        (0x0010, 1 << 13), // Start -> Options
        (0x0040, 1 << 14), // LS -> L3
        (0x0080, 1 << 15), // RS -> R3
    ] {
        if on(x) {
            buttons |= ds4;
        }
    }
    if r.lt > 0.05 {
        buttons |= 1 << 10; // L2
    }
    if r.rt > 0.05 {
        buttons |= 1 << 11; // R2
    }
    DS4Report {
        thumb_lx: byte_axis(r.lx, false),
        thumb_ly: byte_axis(r.ly, true),
        thumb_rx: byte_axis(r.rx, false),
        thumb_ry: byte_axis(r.ry, true),
        buttons,
        special: on(0x0400) as u8, // Guide -> PS
        trigger_l: trigger(r.lt),
        trigger_r: trigger(r.rt),
    }
}

impl Pad {
    pub fn connect(kind: PadKind) -> Result<Self, String> {
        let client = Client::connect()
            .map_err(|e| format!("ViGEmBus not reachable ({e:?}). Is the ViGEmBus driver installed?"))?;
        let target = match kind {
            PadKind::Xbox => {
                let mut t = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
                t.plugin().map_err(|e| format!("Could not plug in the virtual controller ({e:?})"))?;
                t.wait_ready().map_err(|e| format!("Virtual controller not ready ({e:?})"))?;
                Target::Xbox(t, XGamepad::default())
            }
            PadKind::Ds4 => {
                let mut t = DualShock4Wired::new(client, TargetId::DUALSHOCK4_WIRED);
                t.plugin().map_err(|e| format!("Could not plug in the virtual controller ({e:?})"))?;
                t.wait_ready().map_err(|e| format!("Virtual controller not ready ({e:?})"))?;
                Target::Ds4(t, DS4Report::default())
            }
        };
        Ok(Self { target })
    }

    pub fn update(&mut self, r: &Report) {
        match &mut self.target {
            Target::Xbox(t, last) => {
                let g = XGamepad {
                    buttons: XButtons(r.buttons),
                    left_trigger: trigger(r.lt),
                    right_trigger: trigger(r.rt),
                    thumb_lx: axis(r.lx),
                    thumb_ly: axis(r.ly),
                    thumb_rx: axis(r.rx),
                    thumb_ry: axis(r.ry),
                };
                if g != *last {
                    *last = g;
                    let _ = t.update(&g);
                }
            }
            Target::Ds4(t, last) => {
                let g = to_ds4(r);
                if g != *last {
                    *last = g;
                    let _ = t.update(&g);
                }
            }
        }
    }
}

impl Drop for Pad {
    fn drop(&mut self) {
        match &mut self.target {
            Target::Xbox(t, _) => {
                let _ = t.update(&XGamepad::default());
                let _ = t.unplug();
            }
            Target::Ds4(t, _) => {
                let _ = t.update(&DS4Report::default());
                let _ = t.unplug();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ds4_translation() {
        let neutral = to_ds4(&Report::default());
        assert_eq!(neutral, DS4Report::default(), "neutral report = the driver's neutral DS4 report");
        let r = Report { buttons: 0x1000 | 0x0001 | 0x0008 | 0x0400, lt: 0.5, ly: 1.0, lx: -1.0, ..Report::default() };
        let d = to_ds4(&r);
        assert_eq!(d.buttons & 0xF, 1, "up + right = north-east");
        assert!(d.buttons & (1 << 5) != 0, "A -> Cross");
        assert!(d.buttons & (1 << 10) != 0, "LT pressed -> L2");
        assert_eq!((d.thumb_ly, d.thumb_lx), (0, 0), "stick up-left: Y inverted");
        assert_eq!(d.special, 1, "Guide -> PS");
        assert_eq!(d.trigger_l, 128);
    }
}

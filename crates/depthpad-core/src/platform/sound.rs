//! Short, soft sound cues for hotkey switches, audible even over exclusive-fullscreen games:
//! pause falls, resume rises, a profile switch gives one pip per profile number. The tones are
//! synthesised once into in-memory WAVs and played asynchronously.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cue {
    Paused,
    Resumed,
    /// The profile's position in the list, from 1 (pips are capped at 4).
    Profile(usize),
}

const RATE: u32 = 44_100;

/// Notes as (frequency Hz, duration s, silence after s).
fn notes(cue: Cue) -> Vec<(f64, f64, f64)> {
    match cue {
        Cue::Paused => vec![(880.0, 0.07, 0.02), (587.0, 0.09, 0.0)],
        Cue::Resumed => vec![(587.0, 0.07, 0.02), (880.0, 0.09, 0.0)],
        Cue::Profile(n) => vec![(988.0, 0.045, 0.06); n.clamp(1, 4)],
    }
}

/// A mono 16-bit WAV of the cue: sine notes with short fades so they click neither on nor off.
fn wav(cue: Cue) -> Vec<u8> {
    let mut pcm: Vec<i16> = Vec::new();
    for (f, dur, gap) in notes(cue) {
        let n = (dur * RATE as f64) as usize;
        let fade = (0.008 * RATE as f64) as usize;
        for i in 0..n {
            let env = (i.min(n - i) as f64 / fade as f64).min(1.0);
            let s = (2.0 * std::f64::consts::PI * f * i as f64 / RATE as f64).sin();
            pcm.push((s * env * 0.22 * i16::MAX as f64) as i16);
        }
        pcm.extend(std::iter::repeat(0).take((gap * RATE as f64) as usize));
    }
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Play a cue (returns at once). Each cue's WAV is built once and kept: an asynchronous
/// PlaySound reads the memory while it plays.
pub fn play(cue: Cue) {
    static CACHE: OnceLock<Mutex<HashMap<Cue, &'static [u8]>>> = OnceLock::new();
    let bytes: &'static [u8] = *CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .entry(cue)
        .or_insert_with(|| Box::leak(wav(cue).into_boxed_slice()));
    unsafe {
        let _ = PlaySoundW(PCWSTR(bytes.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_is_well_formed() {
        let w = wav(Cue::Profile(2));
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
        assert_eq!(&w[36..40], b"data");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize, w.len() - 44);
        assert!(wav(Cue::Profile(9)).len() == wav(Cue::Profile(4)).len(), "pips capped at 4");
    }
}

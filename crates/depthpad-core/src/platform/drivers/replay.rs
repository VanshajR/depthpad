//! The demo keyboard: plays a recorded session back in real time, over and over, as if a
//! keyboard were connected (analog samples and key events, with the recording's timing). For
//! trying Depthpad and its multi-keyboard behaviour without hardware, and for contributors
//! without the keyboard a recording came from. Enabled with DEPTHPAD_DEMO_KEYBOARD=<file>.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::{Connected, Session, StreamSink, StreamState, StreamStatus};
use crate::device::Model;
use crate::platform::now;
use crate::protocols::Sample;
use crate::recording::{self, Event};

/// The recording the demo keyboard plays, if one is configured and readable.
pub fn configured() -> Option<String> {
    std::env::var("DEPTHPAD_DEMO_KEYBOARD").ok().filter(|p| std::path::Path::new(p).is_file())
}

pub struct Replay {
    stop: Arc<AtomicBool>,
    status: Arc<StreamStatus>,
    thread: Option<JoinHandle<()>>,
}

impl Replay {
    pub fn start(model: &'static Model, sink: Arc<dyn StreamSink>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = StreamStatus::new();
        let (st, sv) = (stop.clone(), status.clone());
        let thread = std::thread::Builder::new()
            .name("depthpad-replay".into())
            .spawn(move || run(model, sink, st, sv))
            .expect("spawn replay thread");
        Self { stop, status, thread: Some(thread) }
    }
}

impl Session for Replay {
    fn status(&self) -> &Arc<StreamStatus> {
        &self.status
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Replay {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(model: &'static Model, sink: Arc<dyn StreamSink>, stop: Arc<AtomicBool>, status: Arc<StreamStatus>) {
    let events = configured()
        .and_then(|p| std::fs::read_to_string(&p).ok().map(|text| (p, text)))
        .map(|(p, text)| (p, recording::parse(&text, model)));
    let Some((path, events)) = events.filter(|(_, e)| !e.is_empty()) else {
        *status.error.lock() = Some("The demo keyboard has no readable recording.".into());
        return;
    };
    let to_usage: BTreeMap<u8, u16> = model.keys.iter().filter(|(_, k)| k.hid != 0).map(|(&p, k)| (p, k.hid)).collect();
    let name = std::path::Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
    sink.connected(Connected { firmware: Some("replay".into()), to_usage, note: Some(format!("demo keyboard playing {name}")), actuation: BTreeMap::new() });
    *status.state.lock() = StreamState::Streaming;
    let start_t = events[0].t();
    while !stop.load(Ordering::SeqCst) {
        let t0 = now();
        let mut held: Vec<u8> = Vec::new();
        for e in &events {
            // wait for the event's moment, waking often enough to stop promptly
            loop {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let wait = (e.t() - start_t) - (now() - t0);
                if wait <= 0.0 {
                    break;
                }
                std::thread::sleep(Duration::from_secs_f64(wait.min(0.05)));
            }
            let t = now();
            match *e {
                Event::Sample { pos, travel, .. } => {
                    status.packets.fetch_add(1, Ordering::Relaxed);
                    sink.sample(Sample { position: pos, travel, stroke: (model.travel_mm * 10.0).round() as u16 }, t);
                }
                Event::Key { pos, down, .. } => {
                    if down {
                        held.push(pos);
                    } else {
                        held.retain(|&p| p != pos);
                    }
                    sink.key(pos, down, t);
                }
            }
        }
        // the end of the recording: let go of everything, pause, play again
        for pos in held {
            sink.key(pos, false, now());
        }
        sink.lost();
        std::thread::sleep(Duration::from_secs(1));
    }
}

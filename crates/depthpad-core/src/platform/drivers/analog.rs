//! Driver for keyboards whose analog protocols come from AnalogSense (`protocols::analogsense`):
//! Wooting, Razer, NuPhy, DrunkDeer, Keychron, Lemokey, MadLions and Bytech-based boards. These
//! report every pressed key, so each key gets real depth; a key that stops being reported is
//! sent as released. Keys still pressed are re-sent every few milliseconds, so the engine treats
//! them as freshly measured. Untested on hardware: written from AnalogSense's working code.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::{Connected, Session, StreamSink, StreamState, StreamStatus};
use crate::device::Model;
use crate::platform::hid::{input_report_ids, HidCollection};
use crate::platform::now;
use crate::protocols::analogsense::{for_driver, AnalogProtocol, Out};
use crate::protocols::Sample;

/// Re-send pressed keys this often, so they count as measured.
const REFRESH: f64 = 0.010;
/// No complete report for this long: analog data can't be trusted.
const CLEAR_AFTER: f64 = 1.0;
/// ... and for this long: start the protocol again.
const RESTART_AFTER: f64 = 2.0;

/// Whether a program with this executable name is running.
fn process_running(names: &[&str]) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = false;
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok && !found {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            let exe = String::from_utf16_lossy(&e.szExeFile[..len]);
            found = names.iter().any(|n| exe.eq_ignore_ascii_case(n));
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
        found
    }
}

/// The collection to talk to: matching IDs, usage and (if the model says) report ID.
fn find(model: &Model) -> io::Result<(String, u16, u16)> {
    let api = hidapi::HidApi::new().map_err(|e| io::Error::other(e.to_string()))?;
    for d in api.device_list() {
        let Some(m) = model.usb_match(d.vendor_id(), d.product_id(), d.usage_page(), d.usage()) else { continue };
        let path = d.path().to_string_lossy().into_owned();
        if let Some(id) = m.report_id {
            if !input_report_ids(&path).contains(&id) {
                continue;
            }
        }
        return Ok((path, d.product_id(), d.usage_page()));
    }
    Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found. Connect it with the USB cable.", model.name)))
}

pub struct AnalogReader {
    stop: Arc<AtomicBool>,
    status: Arc<StreamStatus>,
    thread: Option<JoinHandle<()>>,
}

impl AnalogReader {
    pub fn start(model: &'static Model, sink: Arc<dyn StreamSink>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = StreamStatus::new();
        let (st, sv) = (stop.clone(), status.clone());
        let thread = std::thread::Builder::new()
            .name("depthpad-analog".into())
            .spawn(move || run(model, sink, st, sv))
            .expect("spawn analog thread");
        Self { stop, status, thread: Some(thread) }
    }
}

impl Session for AnalogReader {
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

impl Drop for AnalogReader {
    fn drop(&mut self) {
        self.stop();
    }
}

fn send(dev: &mut HidCollection, outs: Vec<Out>) -> io::Result<()> {
    for o in outs {
        let mut r = Vec::with_capacity(o.data.len() + 1);
        r.push(o.report_id);
        r.extend_from_slice(&o.data);
        dev.write(&r, 100)?;
    }
    Ok(())
}

struct Open {
    dev: HidCollection,
    proto: Box<dyn AnalogProtocol>,
}

fn open(model: &'static Model) -> io::Result<Open> {
    if model.driver == "razer" && !process_running(&["RazerAppEngine.exe", "Razer Synapse 3.exe", "Razer Synapse.exe"]) {
        return Err(io::Error::other(format!("{} needs Razer Synapse running to send analog data.", model.name)));
    }
    let (path, pid, usage_page) = find(model)?;
    let dev = HidCollection::open(&path)?;
    let proto = for_driver(&model.driver, pid, usage_page)
        .ok_or_else(|| io::Error::other(format!("no protocol for driver {}", model.driver)))?;
    Ok(Open { dev, proto })
}

fn run(model: &'static Model, sink: Arc<dyn StreamSink>, stop: Arc<AtomicBool>, status: Arc<StreamStatus>) {
    let stroke = (model.travel_mm * 10.0).round() as u16; // 0.1 mm
    let full = model.travel_mm * 100.0; // 0.01 mm
    let to_usage: BTreeMap<u8, u16> = model.keys.iter().filter(|(_, k)| k.hid != 0).map(|(&p, k)| (p, k.hid)).collect();
    let mut kb: Option<Open> = None;
    let mut active: HashMap<u8, u16> = HashMap::new();
    let (mut last_report, mut last_refresh, mut cleared) = (now(), now(), false);
    let set_state = |s| *status.state.lock() = s;
    while !stop.load(Ordering::SeqCst) {
        let Some(k) = kb.as_mut() else {
            match open(model) {
                Ok(mut o) => {
                    *status.error.lock() = None;
                    let start = o.proto.start();
                    if let Err(e) = send(&mut o.dev, start) {
                        sink.event(format!("{}: {e}", model.name));
                        std::thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                    sink.connected(Connected { firmware: None, to_usage: to_usage.clone(), note: model.note.clone(), actuation: BTreeMap::new() });
                    sink.event(format!("{} connected", model.name));
                    set_state(StreamState::Streaming);
                    (last_report, cleared) = (now(), false);
                    kb = Some(o);
                }
                Err(e) => {
                    let msg = e.to_string();
                    let mut err = status.error.lock();
                    if err.as_deref() != Some(msg.as_str()) {
                        sink.event(msg.clone());
                        *err = Some(msg);
                    }
                    drop(err);
                    set_state(StreamState::Searching);
                    std::thread::sleep(Duration::from_millis(1000));
                }
            }
            continue;
        };
        let read = k.dev.read(5).map(|o| o.map(|d| d.to_vec()));
        let t = now();
        let result: io::Result<()> = (|| {
            if let Some(data) = read? {
                let id = k.dev.last_report_id;
                let (keys, outs) = k.proto.input(id, &data);
                send(&mut k.dev, outs)?;
                if let Some(keys) = keys {
                    status.packets.fetch_add(1, Ordering::Relaxed);
                    last_report = t;
                    cleared = false;
                    set_state(StreamState::Streaming);
                    let mut now_active: HashMap<u8, u16> = HashMap::new();
                    for (usage, v) in keys {
                        // positions are HID usages for these models; Fn and media keys aren't keys here
                        let Ok(pos) = u8::try_from(usage) else { continue };
                        if model.keys.contains_key(&pos) {
                            now_active.insert(pos, (v.clamp(0.0, 1.0) * full).round() as u16);
                        }
                    }
                    for (&pos, _) in active.iter().filter(|(p, _)| !now_active.contains_key(p)) {
                        sink.sample(Sample { position: pos, travel: 0, stroke }, t);
                    }
                    for (&pos, &travel) in &now_active {
                        sink.sample(Sample { position: pos, travel, stroke }, t);
                    }
                    active = now_active;
                    last_refresh = t;
                }
            }
            if t - last_refresh >= REFRESH {
                last_refresh = t;
                for (&pos, &travel) in &active {
                    sink.sample(Sample { position: pos, travel, stroke }, t);
                }
            }
            let outs = k.proto.tick(t);
            send(&mut k.dev, outs)?;
            Ok(())
        })();
        if let Err(e) = result {
            sink.lost();
            active.clear();
            kb = None;
            set_state(StreamState::Searching);
            sink.event(format!("keyboard lost ({e}); analog cleared"));
            continue;
        }
        let silent = t - last_report;
        if silent > CLEAR_AFTER && !cleared {
            sink.lost();
            active.clear();
            cleared = true;
            set_state(StreamState::Silent);
        }
        if silent > RESTART_AFTER {
            status.rearms.fetch_add(1, Ordering::Relaxed);
            last_report = t;
            if let Some(k) = kb.as_mut() {
                let start = k.proto.start();
                if send(&mut k.dev, start).is_err() {
                    kb = None;
                }
            }
        }
    }
}

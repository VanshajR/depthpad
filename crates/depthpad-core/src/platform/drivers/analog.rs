//! Driver for every keyboard with an analog protocol in `protocols` (from AnalogSense and
//! HallJoy): the protocol says what to send, this does the I/O. These report every pressed key,
//! so each key gets real depth; a key that stops being reported is sent as released. Keys still
//! pressed are re-sent every few milliseconds, so the engine treats them as freshly measured.
//! When the protocol learns what the firmware calls itself, the matching family variant is
//! reported (`StreamSink::identified`). Untested on hardware: written from working code.

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
use crate::protocols::analog::{AnalogProtocol, Keys, Out, OutKind};
use crate::protocols::{analog_protocol, UsbInfo};
use crate::protocols::Sample;

/// Re-send pressed keys this often, so they count as measured.
const REFRESH: f64 = 0.010;
/// No complete report for this long: analog data can't be trusted.
const CLEAR_AFTER: f64 = 1.0;
/// ... and for this long: start the protocol again.
const RESTART_AFTER: f64 = 2.0;
/// After the protocol reports a problem, try the keyboard again this much later.
const PROBLEM_RETRY: f64 = 10.0;

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
fn find(model: &Model) -> io::Result<(String, UsbInfo)> {
    let api = hidapi::HidApi::new().map_err(|e| io::Error::other(e.to_string()))?;
    for d in api.device_list() {
        let Some(m) = model.usb_match(d.vendor_id(), d.product_id(), d.usage_page(), d.usage()) else { continue };
        let path = d.path().to_string_lossy().into_owned();
        if let Some(id) = m.report_id {
            if !input_report_ids(&path).contains(&id) {
                continue;
            }
        }
        let product = d.product_string().unwrap_or("").trim().to_string();
        return Ok((path, UsbInfo { vendor_id: d.vendor_id(), product_id: d.product_id(), usage_page: d.usage_page(), product }));
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

/// Send reports; feature reports asked for are read and handed to the protocol, and anything it
/// sends in reply is sent too. Returns the last complete key set seen.
fn send(dev: &mut HidCollection, proto: &mut dyn AnalogProtocol, outs: Vec<Out>) -> io::Result<Option<Keys>> {
    let mut queue: std::collections::VecDeque<Out> = outs.into();
    let mut keys = None;
    let mut sent = 0;
    while let Some(o) = queue.pop_front() {
        sent += 1;
        if sent > 256 {
            return Err(io::Error::other("protocol sent too many reports in a row"));
        }
        let mut r = Vec::with_capacity(o.data.len() + 1);
        r.push(o.report_id);
        r.extend_from_slice(&o.data);
        match o.kind {
            OutKind::Write => {
                dev.write(&r, 100)?;
            }
            OutKind::Feature => dev.set_feature(&r)?,
            OutKind::Pause(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
            OutKind::GetFeature(len) => {
                let data = dev.get_feature(o.report_id, len)?;
                let (k, more) = proto.feature(o.report_id, &data);
                if k.is_some() {
                    keys = k;
                }
                queue.extend(more);
            }
        }
    }
    Ok(keys)
}

struct Open {
    dev: HidCollection,
    /// Where input reports come from when it isn't `dev` (see `AnalogProtocol::input_collection`).
    input: Option<HidCollection>,
    proto: Box<dyn AnalogProtocol>,
}

impl Open {
    fn read(&mut self, timeout_ms: u32) -> io::Result<Option<(u8, Vec<u8>)>> {
        let dev = self.input.as_mut().unwrap_or(&mut self.dev);
        Ok(dev.read(timeout_ms)?.map(|d| d.to_vec()).map(|d| (dev.last_report_id, d)))
    }
}

/// Another collection of the keyboard at `path` with this usage: the one whose path shares the
/// most with it (collections of one device differ only near the end of their paths).
fn sibling(path: &str, vid: u16, pid: u16, (page, usage): (u16, u16)) -> io::Result<String> {
    let api = hidapi::HidApi::new().map_err(|e| io::Error::other(e.to_string()))?;
    let common = |p: &str| path.bytes().zip(p.bytes()).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).count();
    api.device_list()
        .filter(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == page && d.usage() == usage)
        .map(|d| d.path().to_string_lossy().into_owned())
        .filter(|p| p != path)
        .max_by_key(|p| common(p))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the keyboard's input collection is missing"))
}

fn open(model: &'static Model) -> io::Result<Open> {
    if model.driver == "razer" && !process_running(&["RazerAppEngine.exe", "Razer Synapse 3.exe", "Razer Synapse.exe"]) {
        return Err(io::Error::other(format!("{} needs Razer Synapse running to send analog data.", model.name)));
    }
    let (path, usb) = find(model)?;
    let mut dev = HidCollection::open(&path)?;
    crate::device::note_opened(model, dev.vendor_id, dev.product_id);
    let proto = analog_protocol(&model.driver, &usb)
        .ok_or_else(|| io::Error::other(format!("no protocol for driver {}", model.driver)))?;
    if proto.control_channel() {
        dev.use_control_channel();
    }
    // a keyboard that turns out not to stream doesn't need its input collection
    let input = proto
        .input_collection()
        .and_then(|u| sibling(&path, dev.vendor_id, dev.product_id, u).ok())
        .and_then(|p| HidCollection::open(&p).ok());
    Ok(Open { dev, input, proto })
}

fn run(model: &'static Model, sink: Arc<dyn StreamSink>, stop: Arc<AtomicBool>, status: Arc<StreamStatus>) {
    let stroke = (model.travel_mm * 10.0).round() as u16; // 0.1 mm
    let full = model.travel_mm * 100.0; // 0.01 mm
    let to_usage: BTreeMap<u8, u16> = model.keys.iter().filter(|(_, k)| k.hid != 0).map(|(&p, k)| (p, k.hid)).collect();
    let code_pos = model.code_positions();
    let mut kb: Option<Open> = None;
    let mut active: HashMap<u8, u16> = HashMap::new();
    let (mut last_report, mut last_refresh, mut cleared) = (now(), now(), false);
    let mut identity_seen = false;
    let mut retry_at = 0.0;
    let set_state = |s| *status.state.lock() = s;
    let report_error = |msg: String| {
        let mut err = status.error.lock();
        if err.as_deref() != Some(msg.as_str()) {
            sink.event(msg.clone());
            *err = Some(msg);
        }
    };
    // A complete key set: publish it, releasing keys that are no longer in it.
    let publish = |keys: Keys, active: &mut HashMap<u8, u16>, t: f64| {
        let mut now_active: HashMap<u8, u16> = HashMap::new();
        for (code, v) in keys {
            if let Some(&pos) = code_pos.get(&code) {
                now_active.insert(pos, (v.clamp(0.0, 1.0) * full).round() as u16);
            }
        }
        for (&pos, _) in active.iter().filter(|(p, _)| !now_active.contains_key(p)) {
            sink.sample(Sample { position: pos, travel: 0, stroke }, t);
        }
        for (&pos, &travel) in &now_active {
            sink.sample(Sample { position: pos, travel, stroke }, t);
        }
        *active = now_active;
    };
    loop {
        if stop.load(Ordering::SeqCst) {
            if let Some(k) = kb.as_mut() {
                let outs = k.proto.stop();
                let _ = send(&mut k.dev, k.proto.as_mut(), outs);
            }
            break;
        }
        let Some(k) = kb.as_mut() else {
            if now() < retry_at {
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }
            match open(model) {
                Ok(mut o) => {
                    *status.error.lock() = None;
                    let start = o.proto.start();
                    if let Err(e) = send(&mut o.dev, o.proto.as_mut(), start) {
                        sink.event(format!("{}: {e}", model.name));
                        std::thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                    sink.connected(Connected { firmware: None, to_usage: to_usage.clone(), note: model.note.clone(), actuation: BTreeMap::new() });
                    sink.event(format!("{} connected", model.name));
                    set_state(StreamState::Streaming);
                    (last_report, cleared, identity_seen) = (now(), false, false);
                    kb = Some(o);
                }
                Err(e) => {
                    report_error(e.to_string());
                    set_state(StreamState::Searching);
                    std::thread::sleep(Duration::from_millis(1000));
                }
            }
            continue;
        };
        let read = match k.proto.read_timeout_ms() {
            Some(ms) => k.read(ms),
            None => Ok(None),
        };
        let t = now();
        let result: io::Result<()> = (|| {
            let mut complete = None;
            if let Some((id, data)) = read? {
                let (keys, outs) = k.proto.input(id, &data);
                complete = keys;
                if let Some(more) = send(&mut k.dev, k.proto.as_mut(), outs)? {
                    complete = Some(more);
                }
            }
            let outs = k.proto.tick(t);
            if outs.is_empty() && k.proto.read_timeout_ms().is_none() {
                std::thread::sleep(Duration::from_millis(1)); // nothing to read or send
            }
            if let Some(more) = send(&mut k.dev, k.proto.as_mut(), outs)? {
                complete = Some(more);
            }
            if let Some(keys) = complete {
                status.packets.fetch_add(1, Ordering::Relaxed);
                last_report = t;
                cleared = false;
                set_state(StreamState::Streaming);
                publish(keys, &mut active, t);
                last_refresh = t;
            }
            if t - last_refresh >= REFRESH {
                last_refresh = t;
                for (&pos, &travel) in &active {
                    sink.sample(Sample { position: pos, travel, stroke }, t);
                }
            }
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
        if let Some(problem) = k.proto.problem() {
            sink.lost();
            active.clear();
            kb = None;
            set_state(StreamState::Searching);
            report_error(format!("{}: {problem}", model.name));
            retry_at = now() + PROBLEM_RETRY;
            continue;
        }
        if !identity_seen {
            if let Some(name) = k.proto.identity() {
                identity_seen = true;
                match crate::device::variant(model, &name) {
                    Some(v) if v.id != model.id => {
                        sink.event(format!("firmware says \"{name}\": this is a {}", v.name));
                        sink.identified(v);
                    }
                    Some(_) => {}
                    None => sink.event(format!("firmware says \"{name}\"; no exact layout for it yet, showing the {} one", model.name)),
                }
            }
        }
        let silent = if k.proto.event_driven() { 0.0 } else { t - last_report };
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
                if send(&mut k.dev, k.proto.as_mut(), start).is_err() {
                    kb = None;
                }
            }
        }
    }
}

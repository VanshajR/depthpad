//! Driver for Sonix-based AJAZZ keyboards (protocol in `protocols::sonix_ajazz`): admission
//! checks, commands, and the stream thread.
//!
//! Follows HallJoy's MINI60 session handling: admit only one matching device with 65-byte
//! reports whose device-info reply echoes its VID/PID; confirm the stream started; re-arm a
//! silent stream; clear analog after 1 s of silence; reconnect after an I/O error or unplug;
//! confirm the stream stopped on exit. A named mutex gives one owner per keyboard across
//! processes (as AnalogSense's Soup does per device family).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

use super::{Connected, Session, StreamSink, StreamState, StreamStatus};
use crate::device::Model;
use crate::platform::hid::HidCollection;
use crate::platform::{now, wide};
use crate::protocols::sonix_ajazz::{self as protocol, key_layout, DeviceInfo};

struct DeviceLock(HANDLE, bool);

impl DeviceLock {
    fn acquire(name: &str) -> io::Result<Self> {
        let w = wide(name);
        let h = unsafe { CreateMutexW(None, false, PCWSTR(w.as_ptr())) }.map_err(|e| io::Error::other(e.to_string()))?;
        let r = unsafe { WaitForSingleObject(h, 0) };
        let owned = r == WAIT_OBJECT_0 || r == WAIT_ABANDONED;
        Ok(Self(h, owned))
    }
}

impl Drop for DeviceLock {
    fn drop(&mut self) {
        unsafe {
            if self.1 {
                let _ = ReleaseMutex(self.0);
            }
            let _ = CloseHandle(self.0);
        }
    }
}

unsafe impl Send for DeviceLock {}

pub struct Keyboard {
    dev: HidCollection,
    pub info: DeviceInfo,
    _lock: DeviceLock,
}

fn user_error(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, msg.into())
}

impl Keyboard {
    pub fn open(model: &Model) -> io::Result<Self> {
        let api = hidapi::HidApi::new().map_err(|e| io::Error::other(e.to_string()))?;
        let devices: Vec<(String, u16, u16)> = api
            .device_list()
            .filter(|d| model.matches(d.vendor_id(), d.product_id(), d.usage_page(), d.usage()))
            .map(|d| (d.path().to_string_lossy().into_owned(), d.vendor_id(), d.product_id()))
            .collect();
        let name = &model.name;
        match devices.len() {
            0 => return Err(user_error(format!("{name} not found. Connect it with the USB cable."))),
            1 => {}
            _ => return Err(user_error(format!("More than one {name} is connected. Connect only one."))),
        }
        let (path, vid, pid) = &devices[0];
        let lock = DeviceLock::acquire(&format!("Local\\Depthpad-{vid:04X}-{pid:04X}"))?;
        if !lock.1 {
            return Err(user_error("Another Depthpad window or command line is already using the keyboard."));
        }
        let dev = HidCollection::open(path)?;
        let c = &dev.caps;
        if !model.matches(dev.vendor_id, dev.product_id, c.usage_page, c.usage)
            || c.input_len as usize != protocol::REPORT_BYTES
            || c.output_len as usize != protocol::REPORT_BYTES
        {
            return Err(user_error(format!(
                "Unexpected HID collection {:04X}:{:04X} in={} out={}",
                c.usage_page, c.usage, c.input_len, c.output_len
            )));
        }
        let mut kb = Self {
            dev,
            info: DeviceInfo { vid: 0, pid: 0, version: String::new(), manufacturer: 0, product: 0, rt_precision: 0 },
            _lock: lock,
        };
        let reply = kb.command(protocol::GET_DEVICE_INFO, 48, 0, true, 0.4)?;
        let info = reply
            .as_deref()
            .and_then(protocol::parse_device_info)
            .ok_or_else(|| user_error("The keyboard did not answer the device-info request."))?;
        if (info.vid, info.pid) != (kb.dev.vendor_id, kb.dev.product_id) {
            return Err(user_error(format!("Device-info reply does not match this device ({:04X}:{:04X}).", info.vid, info.pid)));
        }
        kb.info = info;
        Ok(kb)
    }

    fn await_reply(&mut self, cmd: u8, len: u8, addr: u16, timeout: f64, accept_stream: bool) -> io::Result<Option<Vec<u8>>> {
        let end = now() + timeout;
        while now() < end {
            if let Some(p) = self.dev.read(20)? {
                if p.len() >= 8 && p[0] == 0x55 {
                    if (accept_stream && p[1] == protocol::STREAM) || protocol::is_reply(p, cmd, len, addr) {
                        return Ok(Some(p.to_vec()));
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn command(&mut self, cmd: u8, len: u8, addr: u16, last: bool, timeout: f64) -> io::Result<Option<Vec<u8>>> {
        self.dev.write(&protocol::request(cmd, len, addr, last, &[]), 250)?;
        self.await_reply(cmd, len, addr, timeout, false)
    }

    /// A command that carries data (e.g. the key list for GET_MAGNETIC_AXIS_STATUS).
    pub fn command_data(&mut self, cmd: u8, len: u8, addr: u16, last: bool, data: &[u8], timeout: f64)
                        -> io::Result<Option<Vec<u8>>> {
        self.dev.write(&protocol::request(cmd, len, addr, last, data), 250)?;
        self.await_reply(cmd, len, addr, timeout, false)
    }

    /// Read `total` bytes of a GET command in 56-byte pieces.
    fn read_block(&mut self, cmd: u8, total: u16) -> io::Result<Option<Vec<u8>>> {
        let mut data = Vec::with_capacity(total as usize);
        let mut addr = 0u16;
        while addr < total {
            let n = 56.min(total - addr) as u8;
            let Some(r) = self.command(cmd, n, addr, addr + n as u16 >= total, 0.4)? else {
                return Ok(None);
            };
            data.extend_from_slice(&r[8..8 + n as usize]);
            addr += n as u16;
        }
        Ok(Some(data))
    }

    /// Each key's actuation point (0.01 mm), as set in the keyboard.
    pub fn actuation(&mut self) -> io::Result<Option<std::collections::BTreeMap<u8, f64>>> {
        let precision = self.info.rt_precision;
        Ok(self.read_block(protocol::GET_ACTUATION, protocol::ACTUATION_BYTES)?.map(|d| protocol::parse_actuation(&d, precision)))
    }

    /// GET_KEY: 128 records of 4 bytes, read in 56-byte pieces.
    pub fn key_records(&mut self) -> io::Result<Option<Vec<[u8; 4]>>> {
        let mut data = Vec::with_capacity(512);
        let mut addr = 0u16;
        while addr < 512 {
            let n = 56.min(512 - addr) as u8;
            let Some(r) = self.command(protocol::GET_KEY, n, addr, addr + n as u16 >= 512, 0.4)? else {
                return Ok(None);
            };
            data.extend_from_slice(&r[8..8 + n as usize]);
            addr += n as u16;
        }
        Ok(Some(data.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()))
    }

    pub fn start_stream(&mut self) -> io::Result<bool> {
        self.dev.write(&protocol::request(protocol::SIMULATION_ON, 0, 0, true, &[]), 250)?;
        Ok(self.await_reply(protocol::SIMULATION_ON, 0, 0, 0.3, true)?.is_some())
    }

    pub fn stop_stream(&mut self) -> io::Result<bool> {
        self.dev.write(&protocol::request(protocol::SIMULATION_OFF, 0, 0, true, &[]), 250)?;
        Ok(self.await_reply(protocol::SIMULATION_OFF, 0, 0, 0.18, false)?.is_some())
    }

    pub fn read(&mut self, timeout_ms: u32) -> io::Result<Option<&[u8]>> {
        self.dev.read(timeout_ms)
    }
}

pub struct StreamReader {
    stop: Arc<AtomicBool>,
    status: Arc<StreamStatus>,
    thread: Option<JoinHandle<()>>,
}

const REARM_AFTER: f64 = 0.5;
const REARM_EVERY: f64 = 1.5;
const CLEAR_AFTER: f64 = 1.0;

impl StreamReader {
    pub fn start(model: &'static Model, sink: Arc<dyn StreamSink>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = StreamStatus::new();
        let (st, sv) = (stop.clone(), status.clone());
        let thread = std::thread::Builder::new()
            .name("depthpad-stream".into())
            .spawn(move || run(model, sink, st, sv))
            .expect("spawn stream thread");
        Self { stop, status, thread: Some(thread) }
    }

}

impl Session for StreamReader {
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

impl Drop for StreamReader {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(model: &'static Model, sink: Arc<dyn StreamSink>, stop: Arc<AtomicBool>, status: Arc<StreamStatus>) {
    let mut kb: Option<Keyboard> = None;
    let (mut last_valid, mut last_rearm, mut cleared) = (now(), now(), false);
    let set_state = |s| *status.state.lock() = s;
    while !stop.load(Ordering::SeqCst) {
        let Some(k) = kb.as_mut() else {
            match Keyboard::open(model) {
                Ok(mut k) => {
                    *status.error.lock() = None;
                    let records = k.key_records().ok().flatten();
                    let (to_usage, _) = key_layout(&model.keys, records.as_deref());
                    let remapped = to_usage.iter().filter(|(p, u)| model.keys[*p].hid != **u).count();
                    let note = if records.is_none() {
                        Some("could not read key assignments; using the factory layout".to_string())
                    } else {
                        (remapped > 0).then(|| format!("{remapped} key(s) remapped on the keyboard; following them"))
                    };
                    let actuation = k.actuation().ok().flatten().unwrap_or_default();
                    sink.connected(Connected { firmware: Some(k.info.version.clone()), to_usage, note, actuation });
                    let ok = k.start_stream().unwrap_or(false);
                    set_state(if ok { StreamState::Streaming } else { StreamState::NoAck });
                    sink.event(format!(
                        "keyboard connected (firmware {}){}",
                        k.info.version,
                        if ok { "" } else { "; stream start not acknowledged, will re-arm" }
                    ));
                    kb = Some(k);
                    (last_valid, last_rearm, cleared) = (now(), now(), false);
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
                    std::thread::sleep(Duration::from_millis(400));
                }
            }
            continue;
        };
        let read = k.read(20);
        let t = now();
        match read {
            Err(e) => {
                sink.lost();
                kb = None;
                set_state(StreamState::Searching);
                sink.event(format!("keyboard lost ({e}); analog cleared"));
                continue;
            }
            Ok(Some(p)) => {
                if let Some(s) = protocol::parse_stream(p) {
                    status.packets.fetch_add(1, Ordering::Relaxed);
                    last_valid = t;
                    cleared = false;
                    set_state(StreamState::Streaming);
                    sink.sample(s, t);
                    continue;
                }
                if p.first() != Some(&0x55) {
                    status.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(None) => {}
        }
        let silent = t - last_valid;
        if silent > CLEAR_AFTER && !cleared {
            sink.lost();
            cleared = true;
            set_state(StreamState::Silent);
        }
        if silent > REARM_AFTER && t - last_rearm > REARM_EVERY {
            status.rearms.fetch_add(1, Ordering::Relaxed);
            last_rearm = t;
            if let Err(e) = kb.as_mut().unwrap().start_stream() {
                sink.lost();
                kb = None;
                set_state(StreamState::Searching);
                sink.event(format!("keyboard lost ({e}); analog cleared"));
            }
        }
    }
    if let Some(mut k) = kb {
        let acked = k.stop_stream().unwrap_or(false);
        sink.event(format!("analog stream off (acknowledged={acked})"));
    }
}

//! depthpad-probe: creates a keyboard report, the information needed to add support for a
//! Hall-effect keyboard to Depthpad. See docs/KEYBOARD_REPORTS.md.
//!
//! It only listens. It sends nothing to the keyboard (no commands, no settings), and records key
//! events and reports only during the guided steps, so nothing typed elsewhere ends up in the
//! report. The report is a JSON file the user can read before sharing it.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use depthpad_core::device::{self, Model, UsbMatch};
use depthpad_core::platform::hid::{input_report_ids, HidCollection};
use depthpad_core::platform::rawinput::DeviceKeys;
use parking_lot::Mutex;
use serde::Serialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Per collection and step, keep at most this many reports (identical repeats are counted, not stored).
const MAX_REPORTS: usize = 6000;

#[derive(Serialize)]
struct Report {
    probe_version: &'static str,
    created_unix: u64,
    windows: String,
    keyboard: KeyboardInfo,
    already_supported_as: Option<String>,
    collections: Vec<CollectionInfo>,
    steps: Vec<StepRecord>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct KeyboardInfo {
    vid: String,
    pid: String,
    manufacturer: Option<String>,
    product: Option<String>,
    release: String,
}

#[derive(Serialize, Clone)]
struct CollectionInfo {
    interface: i32,
    usage_page: String,
    usage: String,
    kind: &'static str,
    input_report_ids: Vec<u8>,
    input_report_bytes: Option<u16>,
    output_report_bytes: Option<u16>,
    /// The raw HID report descriptor, hex.
    report_descriptor: Option<String>,
    /// Why the collection couldn't be listened to, if it couldn't.
    listen_error: Option<String>,
    #[serde(skip)]
    path: String,
}

#[derive(Serialize, Default)]
struct StepRecord {
    step: &'static str,
    instructions: &'static str,
    seconds: f64,
    /// Key events from this keyboard during the step: [ms, HID usage, 1 = down / 0 = up].
    key_events: Vec<(u64, u16, u8)>,
    /// Input reports per collection (by usage page:usage): [ms, report id, hex data, repeats].
    reports: BTreeMap<String, Vec<(u64, u8, String, u32)>>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn kind(page: u16, usage: u16) -> &'static str {
    match (page, usage) {
        (0x01, 0x06) => "keyboard",
        (0x01, 0x02) => "mouse",
        (0x01, 0x80) => "system control",
        (0x0C, _) => "consumer (media keys)",
        (p, _) if p >= 0xFF00 => "vendor-defined",
        _ => "other",
    }
}

fn ask(prompt: &str) -> String {
    print!("{prompt}");
    let _ = io::stdout().flush();
    let mut s = String::new();
    if matches!(io::stdin().lock().read_line(&mut s), Ok(0) | Err(_)) {
        std::process::exit(0); // input closed
    }
    s.trim().to_string()
}

fn windows_version() -> String {
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

struct Listening {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

type Sink = Arc<Mutex<Option<(Instant, StepRecord)>>>;

fn listen(collections: &mut [CollectionInfo], sink: &Sink) -> Listening {
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for c in collections.iter_mut() {
        // Windows keeps keyboard and mouse collections to itself; key events come from Raw Input
        if c.kind == "keyboard" || c.kind == "mouse" {
            continue;
        }
        let mut dev = match HidCollection::open(&c.path) {
            Ok(d) => d,
            Err(e) => {
                c.listen_error = Some(e.to_string());
                continue;
            }
        };
        c.input_report_bytes = Some(dev.caps.input_len);
        c.output_report_bytes = Some(dev.caps.output_len);
        let (stop, sink, name) = (stop.clone(), sink.clone(), format!("{}:{}", c.usage_page, c.usage));
        threads.push(std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let line = match dev.read(50) {
                    Ok(Some(data)) => hex(data),
                    _ => continue,
                };
                let id = dev.last_report_id;
                let mut g = sink.lock();
                let Some((t0, step)) = g.as_mut() else { continue };
                let ms = t0.elapsed().as_millis() as u64;
                let list = step.reports.entry(name.clone()).or_default();
                let repeat = list.last().is_some_and(|last| last.1 == id && last.2 == line);
                if repeat {
                    if let Some(last) = list.last_mut() {
                        last.3 += 1;
                    }
                } else if list.len() < MAX_REPORTS {
                    list.push((ms, id, line, 1));
                }
            }
        }));
    }
    Listening { stop, threads }
}

const STEPS: &[(&str, &str, f64)] = &[
    ("idle", "Don't touch the keyboard.", 4.0),
    ("w_slow", "Press W slowly all the way down, hold it for a second, then let it up slowly. Do it twice.", 10.0),
    ("w_held_d", "Hold W fully down. While holding it, press D slowly part-way and back up a few times. Then let go.", 10.0),
    ("wasd", "Tap W, A, S and D one after another, then press two of them together. Any speed.", 8.0),
];

fn main() {
    println!("Depthpad keyboard report {VERSION}");
    println!("Creates a file describing your keyboard, so support for it can be added to Depthpad.");
    println!("It only listens: nothing is sent to the keyboard and no settings change.\n");

    let api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Couldn't list USB devices: {e}");
            ask("\nPress Enter to close.");
            return;
        }
    };
    // keyboards: devices with a keyboard collection, one entry per VID:PID
    let mut boards: Vec<(u16, u16, String, String, u16)> = Vec::new();
    for d in api.device_list() {
        if (d.usage_page(), d.usage()) == (1, 6) && !boards.iter().any(|b| (b.0, b.1) == (d.vendor_id(), d.product_id())) {
            boards.push((
                d.vendor_id(),
                d.product_id(),
                d.manufacturer_string().unwrap_or("").trim().to_string(),
                d.product_string().unwrap_or("").trim().to_string(),
                d.release_number(),
            ));
        }
    }
    if boards.is_empty() {
        println!("No keyboards found.");
        ask("\nPress Enter to close.");
        return;
    }
    println!("Keyboards:");
    for (i, (vid, pid, m, p, _)) in boards.iter().enumerate() {
        let name = format!("{m} {p}").trim().to_string();
        println!("  {}. {} ({:04X}:{:04X})", i + 1, if name.is_empty() { "Unnamed keyboard" } else { &name }, vid, pid);
    }
    let pick = loop {
        let a = ask(&format!("\nWhich one is the Hall-effect keyboard? [1-{}]: ", boards.len()));
        if let Ok(n) = a.parse::<usize>() {
            if (1..=boards.len()).contains(&n) {
                break n - 1;
            }
        }
    };
    let (vid, pid, manufacturer, product, release) = boards[pick].clone();

    let mut collections: Vec<CollectionInfo> = Vec::new();
    let mut supported = None;
    for d in api.device_list().filter(|d| (d.vendor_id(), d.product_id()) == (vid, pid)) {
        let path = d.path().to_string_lossy().into_owned();
        if collections.iter().any(|c| c.path == path) {
            continue;
        }
        if let Some(m) = device::models().iter().find(|m| m.matches(vid, pid, d.usage_page(), d.usage())) {
            supported = Some(m.name.clone());
        }
        let descriptor = api.open_path(d.path()).ok().and_then(|h| {
            let mut buf = vec![0u8; 4096];
            h.get_report_descriptor(&mut buf).ok().map(|n| hex(&buf[..n]))
        });
        collections.push(CollectionInfo {
            interface: d.interface_number(),
            usage_page: format!("{:04X}", d.usage_page()),
            usage: format!("{:04X}", d.usage()),
            kind: kind(d.usage_page(), d.usage()),
            input_report_ids: input_report_ids(&path),
            input_report_bytes: None,
            output_report_bytes: None,
            report_descriptor: descriptor,
            listen_error: None,
            path,
        });
    }
    println!("\nFound {} HID collections on this keyboard.", collections.len());
    if let Some(s) = &supported {
        println!("Depthpad already supports this keyboard (as {s}). A report still helps if it doesn't work right.");
    }

    // key events from this keyboard only, during steps
    let sink: Sink = Arc::new(Mutex::new(None));
    let mut model: Model = device::fallback().clone();
    model.usb = vec![UsbMatch { vid, pid: Some(pid), usage_page: None, usage: None, report_id: None }];
    let model: &'static Model = Box::leak(Box::new(model));
    let ks = sink.clone();
    let mut keys = DeviceKeys::start(
        model,
        Box::new(move |usage, down, _t| {
            if let Some((t0, step)) = ks.lock().as_mut() {
                step.key_events.push((t0.elapsed().as_millis() as u64, usage, down as u8));
            }
        }),
        Box::new(|| {}),
    );
    let listening = listen(&mut collections, &sink);

    println!("\nNow a few short steps ({} in all). Follow each one while the countdown runs.", STEPS.len());
    println!("If the keyboard's own software is open (configurator, web driver), close it first.\n");
    let mut steps = Vec::new();
    for (i, (id, text, secs)) in STEPS.iter().enumerate() {
        ask(&format!("Step {}/{}: {text}\nPress Enter, then do it.", i + 1, STEPS.len()));
        *sink.lock() = Some((Instant::now(), StepRecord { step: id, instructions: text, seconds: *secs, ..Default::default() }));
        let start = Instant::now();
        while start.elapsed().as_secs_f64() < *secs {
            print!("\r  recording... {:>2} s left ", (*secs - start.elapsed().as_secs_f64()).ceil() as u32);
            let _ = io::stdout().flush();
            std::thread::sleep(Duration::from_millis(200));
        }
        let rec = sink.lock().take().map(|(_, r)| r).unwrap_or_default();
        let n: usize = rec.reports.values().map(Vec::len).sum();
        println!("\r  done: {} key events, {} distinct reports.          \n", rec.key_events.len(), n);
        steps.push(rec);
    }
    listening.stop.store(true, Ordering::SeqCst);
    for t in listening.threads {
        let _ = t.join();
    }
    keys.stop();

    let mut notes = Vec::new();
    let vendor_reports: usize = steps.iter().map(|s| s.reports.values().map(Vec::len).sum::<usize>()).sum();
    if vendor_reports == 0 {
        notes.push("No collection sent anything while keys moved. The keyboard most likely only sends analog data after its \
                    own software switches it on; a USB capture of that software (docs/KEYBOARD_REPORTS.md) is the next step."
            .into());
    }
    let report = Report {
        probe_version: VERSION,
        created_unix: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        windows: windows_version(),
        keyboard: KeyboardInfo {
            vid: format!("{vid:04X}"),
            pid: format!("{pid:04X}"),
            manufacturer: (!manufacturer.is_empty()).then_some(manufacturer),
            product: (!product.is_empty()).then_some(product.clone()),
            release: format!("{:x}.{:02x}", release >> 8, release & 0xFF),
        },
        already_supported_as: supported,
        collections,
        steps,
        notes,
    };
    let name = format!("depthpad-report-{vid:04X}-{pid:04X}.json");
    let dir = std::env::var("USERPROFILE").map(|h| std::path::PathBuf::from(h).join("Desktop")).ok().filter(|d| d.is_dir())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let path = dir.join(&name);
    match std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap_or_default()) {
        Ok(()) => {
            println!("Saved: {}", path.display());
            println!("\nIt holds the keyboard's USB details and what it sent during the steps; no other typing.");
            println!("Attach it to a \"Keyboard support request\" on Depthpad's GitHub page:");
            println!("  https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml");
        }
        Err(e) => println!("Couldn't save {}: {e}", path.display()),
    }
    ask("\nPress Enter to close.");
}

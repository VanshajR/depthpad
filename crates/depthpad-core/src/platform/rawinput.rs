//! Per-device key events via Raw Input. Only events from the active model's keyboard (matched by
//! the VID/PID in its device path) are passed on, so other keyboards never affect the controller.
//! Observes, never blocks.
//!
//! Raw Input registration is per process and per usage: the last window to register for keyboards
//! gets them. Other code in the process (tao, the windowing library under Tauri, registers its
//! event-loop window at start-up) can therefore take keyboard input away from us silently, which
//! leaves only the analog stream and loses every key the stream doesn't follow. A timer checks the
//! registration four times a second and takes it back.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRegisteredRawInputDevices, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDI_DEVICENAME, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetTimer, TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_INPUT,
    WM_INPUT_DEVICE_CHANGE, WM_TIMER, WNDCLASSW,
};

use super::{now, scancodes, wide};
use crate::device::Model;

const RI_KEY_BREAK: u16 = 1;
const RI_KEY_E0: u16 = 2;
const GIDC_REMOVAL: usize = 2;
const CHECK_TIMER: usize = 1;

pub type KeyFn = Box<dyn Fn(u16, bool, f64) + Send>;
pub type RemovedFn = Box<dyn Fn() + Send>;

struct State {
    on_key: KeyFn,
    on_removed: RemovedFn,
    model: &'static Model,
    names: HashMap<isize, String>,
    events: Arc<AtomicU64>,
    reclaims: Arc<AtomicU64>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub struct DeviceKeys {
    hwnd: Arc<AtomicIsize>,
    pub events: Arc<AtomicU64>,
    /// How many times another window took keyboard Raw Input and we took it back.
    pub reclaims: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
}

fn register(win: HWND) -> bool {
    let rid = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 6, dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY, hwndTarget: win };
    unsafe { RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32).is_ok() }
}

/// Whether keyboard Raw Input for this process is still delivered to `win`.
fn registered_to(win: HWND) -> bool {
    let size = std::mem::size_of::<RAWINPUTDEVICE>() as u32;
    let mut list = [RAWINPUTDEVICE::default(); 16];
    let mut n = list.len() as u32;
    let got = unsafe { GetRegisteredRawInputDevices(Some(list.as_mut_ptr()), &mut n, size) };
    if got == u32::MAX {
        return true; // can't tell; don't churn
    }
    list[..got as usize].iter().any(|d| d.usUsagePage == 1 && d.usUsage == 6 && d.hwndTarget == win)
}

fn device_name(handle: HANDLE) -> String {
    unsafe {
        let mut size = 0u32;
        GetRawInputDeviceInfoW(Some(handle), RIDI_DEVICENAME, None, &mut size);
        let mut buf = vec![0u16; size as usize + 1];
        GetRawInputDeviceInfoW(Some(handle), RIDI_DEVICENAME, Some(buf.as_mut_ptr() as *mut c_void), &mut size);
        String::from_utf16_lossy(&buf).trim_end_matches('\0').to_lowercase()
    }
}

unsafe fn on_input(lparam: LPARAM) {
    let mut ri = RAWINPUT::default();
    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
    let n = GetRawInputData(
        HRAWINPUT(lparam.0 as *mut c_void),
        RID_INPUT,
        Some(&mut ri as *mut RAWINPUT as *mut c_void),
        &mut size,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    );
    if n == u32::MAX || ri.header.dwType != RIM_TYPEKEYBOARD.0 {
        return;
    }
    if ri.header.hDevice.is_invalid() {
        // synthetic input; counted for the hook watchdog only when testing with synthetic keys
        if super::hook::accept_injected() {
            super::note_raw_key(now());
        }
        return;
    }
    let kb = ri.data.keyboard;
    super::note_raw_key(now());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(st) = s.as_mut() else { return };
        let key = ri.header.hDevice.0 as isize;
        let name = st.names.entry(key).or_insert_with(|| device_name(ri.header.hDevice)).clone();
        if !st.model.owns_path(&name) {
            return;
        }
        let code = scancodes::code(kb.MakeCode, kb.Flags & RI_KEY_E0 != 0);
        if let Some(usage) = scancodes::to_hid(code) {
            st.events.fetch_add(1, Ordering::Relaxed);
            (st.on_key)(usage, kb.Flags & RI_KEY_BREAK == 0, now());
        }
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_INPUT => on_input(lparam),
            WM_TIMER if wparam.0 == CHECK_TIMER => {
                if !registered_to(hwnd) && register(hwnd) {
                    STATE.with(|s| {
                        if let Some(st) = s.borrow().as_ref() {
                            st.reclaims.fetch_add(1, Ordering::Relaxed);
                        }
                    });
                }
                return LRESULT(0);
            }
            WM_INPUT_DEVICE_CHANGE if wparam.0 == GIDC_REMOVAL => {
                // the name is no longer readable after removal; use the cached one
                STATE.with(|s| {
                    if let Some(st) = s.borrow_mut().as_mut() {
                        let removed = st.names.remove(&(lparam.0 as isize)).unwrap_or_default();
                        if st.model.owns_path(&removed) {
                            (st.on_removed)();
                        }
                    }
                });
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

impl DeviceKeys {
    pub fn start(model: &'static Model, on_key: KeyFn, on_removed: RemovedFn) -> Self {
        let hwnd = Arc::new(AtomicIsize::new(0));
        let events = Arc::new(AtomicU64::new(0));
        let reclaims = Arc::new(AtomicU64::new(0));
        let (h, ev, rc) = (hwnd.clone(), events.clone(), reclaims.clone());
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("depthpad-rawinput".into())
            .spawn(move || unsafe {
                STATE.with(|s| {
                    *s.borrow_mut() = Some(State {
                        on_key,
                        on_removed,
                        model,
                        names: HashMap::new(),
                        events: ev,
                        reclaims: rc,
                    })
                });
                let inst = GetModuleHandleW(None).unwrap_or_default();
                let class = wide("DepthpadRawInput");
                let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: PCWSTR(class.as_ptr()), ..Default::default() };
                RegisterClassW(&wc);
                let Ok(win) = CreateWindowExW(
                    WINDOW_EX_STYLE(0), PCWSTR(class.as_ptr()), PCWSTR(class.as_ptr()), WINDOW_STYLE(0),
                    0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(inst.into()), None,
                ) else {
                    let _ = ready_tx.send(());
                    return;
                };
                h.store(win.0 as isize, Ordering::SeqCst);
                register(win);
                SetTimer(Some(win), CHECK_TIMER, 250, None);
                let _ = ready_tx.send(());
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .expect("spawn raw input thread");
        let _ = ready_rx.recv();
        Self { hwnd, events, reclaims, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        let h = self.hwnd.swap(0, Ordering::SeqCst);
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut c_void)), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for DeviceKeys {
    fn drop(&mut self) {
        self.stop();
    }
}

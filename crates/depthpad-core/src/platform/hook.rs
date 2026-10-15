//! Optional key blocking and the pause hotkey: a WH_KEYBOARD_LL hook driven by `routes::Router`.
//!
//! Measured: a key blocked here never reaches Raw Input, so blocked presses are reported to the
//! engine from the hook itself. The hook cannot tell keyboards apart, so with blocking on, a
//! bound key on another keyboard also counts (HallJoy has the same limit). The hook procedure
//! does no allocation or I/O beyond the callbacks; Windows removes hooks that answer too slowly.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::Mutex;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, MapVirtualKeyW, MAPVK_VK_TO_VSC_EX};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, PostThreadMessageW,
    SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

use super::{last_raw_key, now, scancodes};
use crate::routes::Router;

const LLKHF_EXTENDED: u32 = 0x01;
const LLKHF_INJECTED: u32 = 0x10;

/// When the hook last saw a physical key event (f64 bits of `now()`), for the watchdog.
static LAST_HOOK_KEY: AtomicU64 = AtomicU64::new(0);
/// Raw Input heard a key this long after the hook last did: the hook is gone.
const DEAD_AFTER: f64 = 0.3;

pub type KeyFn = Box<dyn Fn(u16, bool, f64) + Send>;
pub type ToggleFn = Box<dyn Fn(bool) + Send>;
pub type FireFn = Box<dyn Fn(usize) + Send>;

struct HookState {
    router: Router,
    on_key: KeyFn,
    on_toggle: ToggleFn,
    on_fire: FireFn,
}

static STATE: Mutex<Option<HookState>> = parking_lot::const_mutex(None);

/// Testing only: with DEPTHPAD_HOOK_ACCEPT_INJECTED=1, synthetic (SendInput) keys count too, so
/// the hotkey and blocking can be exercised without a person at the keyboard.
fn trace(msg: &str) {
    if accept_injected() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("depthpad-hook.log")) {
            let _ = writeln!(f, "{:.3} {msg}", now());
        }
    }
}

pub(crate) fn accept_injected() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHPAD_HOOK_ACCEPT_INJECTED").is_ok_and(|v| v == "1"))
}

fn own_foreground() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let msg = wparam.0 as u32;
        if code == 0 && matches!(msg, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP) {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            if k.flags.0 & LLKHF_INJECTED == 0 || accept_injected() {
                LAST_HOOK_KEY.store(now().to_bits(), Ordering::Relaxed);
            }
            if k.flags.0 & LLKHF_INJECTED == 0 || accept_injected() {
                let code_ = scancodes::code(k.scanCode as u16, k.flags.0 & LLKHF_EXTENDED != 0);
                if let Some(usage) = scancodes::to_hid(code_) {
                    let down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
                    let fg = own_foreground();
                    if let Some(st) = STATE.lock().as_mut() {
                        let d = st.router.route(usage, down, fg);
                        if let Some(p) = d.toggled {
                            (st.on_toggle)(p);
                        }
                        if let Some(i) = d.fired {
                            (st.on_fire)(i);
                        }
                        if d.report {
                            (st.on_key)(usage, down, now());
                        }
                        if d.swallow {
                            return LRESULT(1);
                        }
                    }
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

/// Watchdog: if Raw Input has heard physical keys the hook never saw, Windows removed the hook;
/// install a fresh one. Returns the new hook when it did.
unsafe fn check_hook(old: HHOOK, inst: Option<windows::Win32::Foundation::HMODULE>) -> Option<HHOOK> {
    let hook_last = f64::from_bits(LAST_HOOK_KEY.load(Ordering::Relaxed));
    let raw_last = last_raw_key();
    if raw_last - hook_last <= DEAD_AFTER || now() - raw_last < DEAD_AFTER {
        return None; // healthy, or Raw Input's event is too recent to judge
    }
    let _ = UnhookWindowsHookEx(old);
    let new = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), inst.map(Into::into), 0).ok()?;
    // keys pressed while there was no hook: start again from what Windows says is held
    if let Some(st) = STATE.lock().as_mut() {
        st.router.reset_held();
        for u in held_now() {
            st.router.seed_held(u);
        }
    }
    LAST_HOOK_KEY.store(now().to_bits(), Ordering::Relaxed);
    trace("hook reinstalled");
    Some(new)
}

/// Keys Windows says are down right now, as HID usages.
fn held_now() -> Vec<u16> {
    let mut out = Vec::new();
    unsafe {
        for vk in 8u32..255 {
            if (GetAsyncKeyState(vk as i32) as u16) & 0x8000 != 0 {
                let sc = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC_EX) as u16;
                if let Some(u) = scancodes::to_hid(scancodes::code(sc, sc & 0xFF00 == 0xE000)) {
                    out.push(u);
                }
            }
        }
    }
    out
}

pub struct KeyBlocker {
    /// Times Windows removed the hook (it does that silently to hooks that answer too slowly,
    /// e.g. while the machine is busy) and the watchdog put it back.
    pub reinstalls: Arc<AtomicU64>,
    thread_id: std::sync::Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl KeyBlocker {
    /// Installs the hook. `bound` and the hotkeys are HID usages; hotkey 0 is pause.
    pub fn start(bound: HashSet<u16>, block: bool, block_all: bool, allow_alt_tab: bool, hotkeys: Vec<Vec<u16>>, on_key: KeyFn,
                 on_toggle: ToggleFn, on_fire: FireFn) -> Result<Self, String> {
        let mut router = Router::new(bound, block, allow_alt_tab, hotkeys);
        router.block_all = block_all;
        for u in held_now() {
            router.seed_held(u);
        }
        trace("start");
        *STATE.lock() = Some(HookState { router, on_key, on_toggle, on_fire });
        let tid = std::sync::Arc::new(AtomicU32::new(0));
        let t2 = tid.clone();
        let reinstalls = Arc::new(AtomicU64::new(0));
        let r2 = reinstalls.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("depthpad-hook".into())
            .spawn(move || unsafe {
                t2.store(GetCurrentThreadId(), Ordering::SeqCst);
                // Windows drops low-level hooks that answer slowly; don't queue behind other work
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
                let inst = GetModuleHandleW(None).ok();
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), inst.map(Into::into), 0);
                trace(&format!("SetWindowsHookExW -> {:?}", hook.as_ref().map(|_| ()).map_err(|e| e.to_string())));
                let _ = tx.send(hook.as_ref().map(|_| ()).map_err(|e| e.to_string()));
                let Ok(mut hook) = hook else { return };
                LAST_HOOK_KEY.store(now().to_bits(), Ordering::Relaxed);
                SetTimer(None, 0, 1000, None);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_TIMER {
                        if let Some(h) = check_hook(hook, inst) {
                            hook = h;
                            r2.fetch_add(1, Ordering::Relaxed);
                        }
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(hook);
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())??;
        Ok(Self { reinstalls, thread_id: tid, thread: Some(thread) })
    }

    pub fn configure(&self, bound: HashSet<u16>, block: bool, block_all: bool, allow_alt_tab: bool, hotkeys: Vec<Vec<u16>>) {
        if let Some(st) = STATE.lock().as_mut() {
            st.router.bound = bound;
            st.router.block = block;
            st.router.block_all = block_all;
            st.router.allow_alt_tab = allow_alt_tab;
            st.router.hotkeys = hotkeys;
        }
    }

    pub fn set_paused(&self, paused: bool) {
        if let Some(st) = STATE.lock().as_mut() {
            st.router.paused = paused;
        }
    }

    pub fn stop(&mut self) {
        trace("stop");
        let tid = self.thread_id.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        *STATE.lock() = None;
    }
}

impl Drop for KeyBlocker {
    fn drop(&mut self) {
        self.stop();
    }
}

//! A small on-screen badge ("Racing", "Paused") for hotkey switches made while a game is in
//! front: a native, topmost, click-through window that never takes focus, drawn with GDI and
//! faded out. No WebView, so it costs next to nothing. Like any normal window it can't appear
//! over a game in exclusive fullscreen (borderless/windowed games are fine).

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;

use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, InvalidateRect, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetMonitorInfoW, MonitorFromPoint, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, FW_NORMAL, FW_SEMIBOLD, HFONT, MONITORINFO, MONITOR_DEFAULTTOPRIMARY, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowRect,
    KillTimer, PostMessageW, RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow,
    TranslateMessage, HWND_TOPMOST, LWA_ALPHA, MSG, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WM_APP, WM_PAINT,
    WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use super::wide;

const W: i32 = 280;
const H: i32 = 64;
const WM_SHOW: u32 = WM_APP + 1;
const FADE_TIMER: usize = 1;
const SHOW_MS: u32 = 1300;
const FADE_STEPS: u32 = 8;

struct Text {
    title: String,
    detail: String,
}

static TEXT: Mutex<Text> = parking_lot::const_mutex(Text { title: String::new(), detail: String::new() });
static HWND_: AtomicIsize = AtomicIsize::new(0);
static STEP: AtomicIsize = AtomicIsize::new(0);
/// Display scale in percent for the window's monitor (100 = 96 dpi).
static SCALE: AtomicIsize = AtomicIsize::new(100);

/// A length in 96-dpi pixels, scaled for the display.
fn px(v: i32) -> i32 {
    v * SCALE.load(Ordering::Relaxed) as i32 / 100
}

/// Show the badge (from any thread). The first call starts the overlay's thread.
pub fn show(title: &str, detail: &str) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("depthpad-overlay".into())
            .spawn(move || run(tx))
            .expect("spawn overlay thread");
        let _ = rx.recv();
    });
    *TEXT.lock() = Text { title: title.into(), detail: detail.into() };
    let h = HWND_.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_SHOW, WPARAM(0), LPARAM(0));
        }
    }
}

fn run(ready: std::sync::mpsc::Sender<()>) {
    unsafe {
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let class = wide("DepthpadOverlay");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: PCWSTR(class.as_ptr()), ..Default::default() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(class.as_ptr()),
            PCWSTR(class.as_ptr()),
            WS_POPUP,
            0, 0, W, H, None, None, Some(inst.into()), None,
        );
        let Ok(hwnd) = hwnd else {
            let _ = ready.send(());
            return;
        };
        HWND_.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = ready.send(());
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Top centre of the monitor the foreground window (the game) is on.
unsafe fn place(hwnd: HWND) {
    let fg = GetForegroundWindow();
    let mut r = RECT::default();
    let center = if !fg.is_invalid() && GetWindowRect(fg, &mut r).is_ok() {
        POINT { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 }
    } else {
        POINT { x: 0, y: 0 }
    };
    let mon = MonitorFromPoint(center, MONITOR_DEFAULTTOPRIMARY);
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = GetMonitorInfoW(mon, &mut mi);
    let m = mi.rcMonitor;
    let x = m.left + (m.right - m.left - px(W)) / 2;
    let y = m.top + (m.bottom - m.top) / 14;
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, px(W), px(H), SWP_NOACTIVATE);
    // the DPI is the monitor's now that the window is on it
    let dpi = GetDpiForWindow(hwnd).max(96) as isize;
    if dpi * 100 / 96 != SCALE.load(Ordering::Relaxed) {
        SCALE.store(dpi * 100 / 96, Ordering::Relaxed);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), m.left + (m.right - m.left - px(W)) / 2, y, px(W), px(H), SWP_NOACTIVATE);
    }
    let rgn = CreateRoundRectRgn(0, 0, px(W) + 1, px(H) + 1, px(18), px(18));
    SetWindowRgn(hwnd, Some(rgn), false);
}

fn font(height: i32, weight: u32) -> HFONT {
    let face = wide("Segoe UI");
    unsafe { CreateFontW(height, 0, 0, 0, weight as i32, 0, 0, 0, Default::default(), Default::default(),
                         Default::default(), Default::default(), Default::default(), PCWSTR(face.as_ptr())) }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SHOW => {
                place(hwnd);
                STEP.store(0, Ordering::SeqCst);
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 240, LWA_ALPHA);
                let _ = InvalidateRect(Some(hwnd), None, true);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = KillTimer(Some(hwnd), FADE_TIMER);
                SetTimer(Some(hwnd), FADE_TIMER, SHOW_MS, None);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == FADE_TIMER => {
                let step = STEP.fetch_add(1, Ordering::SeqCst) as u32 + 1;
                if step == 1 {
                    let _ = KillTimer(Some(hwnd), FADE_TIMER);
                    SetTimer(Some(hwnd), FADE_TIMER, 25, None);
                }
                if step >= FADE_STEPS {
                    let _ = KillTimer(Some(hwnd), FADE_TIMER);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    let alpha = 240 - (240 * step / FADE_STEPS) as u8;
                    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let bg = CreateSolidBrush(COLORREF(0x00_1d_1a_17)); // #171a1d, BGR
                let all = RECT { left: 0, top: 0, right: px(W), bottom: px(H) };
                FillRect(dc, &all, bg);
                let _ = DeleteObject(bg.into());
                SetBkMode(dc, TRANSPARENT);
                let text = TEXT.lock();
                let has_detail = !text.detail.is_empty();
                let big = font(-px(22), FW_SEMIBOLD.0);
                let old = SelectObject(dc, big.into());
                SetTextColor(dc, COLORREF(0x00_f2_f2_f2));
                let mut t: Vec<u16> = text.title.encode_utf16().collect();
                let mut r = if has_detail { RECT { left: px(12), top: px(9), right: px(W - 12), bottom: px(38) } } else { all };
                DrawTextW(dc, &mut t, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                if has_detail {
                    let small = font(-px(13), FW_NORMAL.0);
                    SelectObject(dc, small.into());
                    SetTextColor(dc, COLORREF(0x00_a6_a6_a6));
                    let mut d: Vec<u16> = text.detail.encode_utf16().collect();
                    let mut r = RECT { left: px(12), top: px(37), right: px(W - 12), bottom: px(H - 8) };
                    DrawTextW(dc, &mut d, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    let _ = DeleteObject(small.into());
                }
                SelectObject(dc, old);
                let _ = DeleteObject(big.into());
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

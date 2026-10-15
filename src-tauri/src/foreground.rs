//! Which program is in front, and the programs with windows, for App linking (a profile that
//! switches on by itself when a game is in front).

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

/// The executable path of a process, if Windows lets us ask (games running as administrator
/// may not).
fn exe_of(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// File name, lower case: what links are matched on ("gta5.exe").
pub fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_ascii_lowercase()
}

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

/// The executable name of the program in front.
pub fn foreground_exe() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    exe_of(pid_of(hwnd)).map(|p| exe_name(&p))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningApp {
    pub exe: String,
    pub title: String,
}

/// Programs that have a visible, titled window, one entry per executable.
pub fn running_apps() -> Vec<RunningApp> {
    let mut windows: Vec<HWND> = Vec::new();
    unsafe extern "system" fn collect(hwnd: HWND, l: LPARAM) -> windows::core::BOOL {
        let list = &mut *(l.0 as *mut Vec<HWND>);
        if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
            list.push(hwnd);
        }
        true.into()
    }
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize));
    }
    let own = std::process::id();
    let mut out: Vec<RunningApp> = Vec::new();
    for hwnd in windows {
        let pid = pid_of(hwnd);
        if pid == own {
            continue;
        }
        let Some(exe) = exe_of(pid).map(|p| exe_name(&p)) else { continue };
        if ["explorer.exe", "textinputhost.exe", "applicationframehost.exe", "shellexperiencehost.exe"].contains(&exe.as_str())
            || out.iter().any(|a| a.exe == exe)
        {
            continue;
        }
        let mut buf = vec![0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
        out.push(RunningApp { exe, title: String::from_utf16_lossy(&buf[..n]) });
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

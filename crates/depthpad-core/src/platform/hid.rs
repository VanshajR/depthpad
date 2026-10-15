//! One HID top-level collection opened the way HallJoy's MINI60 session does
//! (aula_mini60_diagnostic.cpp): overlapped I/O, a 512-report input queue, report sizes from the
//! descriptor, and HidD_SetOutputReport as a second route when an interrupt write fails.

use std::ffi::c_void;
use std::io;

use windows::core::PCWSTR;
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetPreparsedData, HidD_SetNumInputBuffers,
    HidD_SetOutputReport, HidP_GetCaps, HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use super::wide;

pub const INPUT_BUFFERS: u32 = 512;

pub struct Caps {
    pub usage_page: u16,
    pub usage: u16,
    pub input_len: u16,
    pub output_len: u16,
}

pub struct HidCollection {
    handle: HANDLE,
    pub vendor_id: u16,
    pub product_id: u16,
    pub caps: Caps,
    pub input_buffers: bool,
    read_buf: Box<[u8]>,
    read_ov: Box<OVERLAPPED>,
    read_pending: bool,
    write_ov: Box<OVERLAPPED>,
    use_output_report: bool,
    /// The report ID of the last report `read` returned.
    pub last_report_id: u8,
}

// The handle and event objects are used from one thread at a time (the stream thread).
unsafe impl Send for HidCollection {}

fn last_error(what: &str) -> io::Error {
    let e = io::Error::last_os_error();
    io::Error::new(e.kind(), format!("{what}: {e}"))
}

impl HidCollection {
    pub fn open(path: &str) -> io::Result<Self> {
        let w = wide(path);
        let handle = unsafe {
            CreateFileW(
                PCWSTR(w.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            )
        }
        .map_err(|e| io::Error::other(format!("open: {e}")))?;
        let mut attr = HIDD_ATTRIBUTES { Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32, ..Default::default() };
        let mut caps = HIDP_CAPS::default();
        unsafe {
            if !HidD_GetAttributes(handle, &mut attr) {
                let _ = CloseHandle(handle);
                return Err(last_error("HidD_GetAttributes"));
            }
            let mut prep = PHIDP_PREPARSED_DATA::default();
            if !HidD_GetPreparsedData(handle, &mut prep) {
                let _ = CloseHandle(handle);
                return Err(last_error("HidD_GetPreparsedData"));
            }
            let status = HidP_GetCaps(prep, &mut caps);
            let _ = HidD_FreePreparsedData(prep);
            if status != HIDP_STATUS_SUCCESS {
                let _ = CloseHandle(handle);
                return Err(io::Error::other("HidP_GetCaps failed"));
            }
        }
        let input_buffers = unsafe { HidD_SetNumInputBuffers(handle, INPUT_BUFFERS) };
        let event = || unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.map_err(|e| io::Error::other(e.to_string()));
        let read_ov = Box::new(OVERLAPPED { hEvent: event()?, ..Default::default() });
        let write_ov = Box::new(OVERLAPPED { hEvent: event()?, ..Default::default() });
        Ok(Self {
            handle,
            vendor_id: attr.VendorID,
            product_id: attr.ProductID,
            caps: Caps {
                usage_page: caps.UsagePage,
                usage: caps.Usage,
                input_len: caps.InputReportByteLength,
                output_len: caps.OutputReportByteLength,
            },
            input_buffers,
            read_buf: vec![0u8; caps.InputReportByteLength.max(1) as usize].into_boxed_slice(),
            read_ov,
            read_pending: false,
            write_ov,
            use_output_report: false,
            last_report_id: 0,
        })
    }

    /// One input report without its report ID byte, or None on timeout. The read stays pending
    /// across timeouts instead of being cancelled, so a report completing later is never lost.
    pub fn read(&mut self, timeout_ms: u32) -> io::Result<Option<&[u8]>> {
        unsafe {
            if !self.read_pending {
                let _ = ResetEvent(self.read_ov.hEvent);
                if let Err(e) = ReadFile(self.handle, Some(&mut self.read_buf), None, Some(&mut *self.read_ov)) {
                    if e.code() != ERROR_IO_PENDING.to_hresult() {
                        return Err(io::Error::other(format!("read: {e}")));
                    }
                }
                self.read_pending = true;
            }
            if WaitForSingleObject(self.read_ov.hEvent, timeout_ms) != WAIT_OBJECT_0 {
                return Ok(None);
            }
            self.read_pending = false;
            let mut n = 0u32;
            GetOverlappedResult(self.handle, &*self.read_ov, &mut n, false)
                .map_err(|e| io::Error::other(format!("read: {e}")))?;
            let n = n as usize;
            self.last_report_id = if n > 0 { self.read_buf[0] } else { 0 };
            Ok(Some(if n > 1 { &self.read_buf[1..n] } else { &[] }))
        }
    }

    /// Write one output report (report ID first). Falls back to HidD_SetOutputReport when an
    /// interrupt write fails or does not complete within `timeout_ms`.
    pub fn write(&mut self, report: &[u8], timeout_ms: u32) -> io::Result<&'static str> {
        let mut buf = report.to_vec();
        buf.resize((self.caps.output_len as usize).max(report.len()), 0);
        unsafe {
            if !self.use_output_report {
                let _ = ResetEvent(self.write_ov.hEvent);
                let started = match WriteFile(self.handle, Some(&buf), None, Some(&mut *self.write_ov)) {
                    Ok(()) => true,
                    Err(e) => e.code() == ERROR_IO_PENDING.to_hresult(),
                };
                if started {
                    let mut n = 0u32;
                    if WaitForSingleObject(self.write_ov.hEvent, timeout_ms) == WAIT_OBJECT_0 {
                        if GetOverlappedResult(self.handle, &*self.write_ov, &mut n, false).is_ok() {
                            return Ok("write");
                        }
                    } else {
                        let _ = CancelIoEx(self.handle, Some(&*self.write_ov));
                        let _ = GetOverlappedResult(self.handle, &*self.write_ov, &mut n, true);
                    }
                }
                self.use_output_report = true;
            }
            if HidD_SetOutputReport(self.handle, buf.as_ptr() as *const c_void, buf.len() as u32) {
                Ok("output-report")
            } else {
                Err(last_error("write"))
            }
        }
    }
}

/// The input report IDs a collection declares (opened without access, so it works for
/// collections Windows keeps for itself too).
pub fn input_report_ids(path: &str) -> Vec<u8> {
    use windows::Win32::Devices::HumanInterfaceDevice::{HidP_GetButtonCaps, HidP_GetValueCaps, HidP_Input, HIDP_BUTTON_CAPS, HIDP_VALUE_CAPS};
    let w = wide(path);
    let Ok(handle) = (unsafe { CreateFileW(PCWSTR(w.as_ptr()), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, Default::default(), None) }) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    unsafe {
        let mut prep = PHIDP_PREPARSED_DATA::default();
        if HidD_GetPreparsedData(handle, &mut prep) {
            let mut caps = HIDP_CAPS::default();
            if HidP_GetCaps(prep, &mut caps) == HIDP_STATUS_SUCCESS {
                let mut n = caps.NumberInputValueCaps;
                let mut vc = vec![HIDP_VALUE_CAPS::default(); n as usize];
                if n > 0 && HidP_GetValueCaps(HidP_Input, vc.as_mut_ptr(), &mut n, prep) == HIDP_STATUS_SUCCESS {
                    ids.extend(vc[..n as usize].iter().map(|c| c.ReportID));
                }
                let mut n = caps.NumberInputButtonCaps;
                let mut bc = vec![HIDP_BUTTON_CAPS::default(); n as usize];
                if n > 0 && HidP_GetButtonCaps(HidP_Input, bc.as_mut_ptr(), &mut n, prep) == HIDP_STATUS_SUCCESS {
                    ids.extend(bc[..n as usize].iter().map(|c| c.ReportID));
                }
            }
            let _ = HidD_FreePreparsedData(prep);
        }
        let _ = CloseHandle(handle);
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

impl Drop for HidCollection {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelIoEx(self.handle, None);
            if self.read_pending {
                let mut n = 0u32;
                let _ = GetOverlappedResult(self.handle, &*self.read_ov, &mut n, true);
            }
            let _ = CloseHandle(self.read_ov.hEvent);
            let _ = CloseHandle(self.write_ov.hEvent);
            let _ = CloseHandle(self.handle);
        }
    }
}

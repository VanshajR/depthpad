"""Direct Windows HID access for one collection.

hidapi is used only to enumerate paths. This module opens the collection the
way HallJoy's MINI60 session does (src/HallJoyProject/HallJoy/aula_mini60_diagnostic.cpp):
overlapped I/O, a 512-report input queue so a stalled reader loses nothing,
report sizes read from the HID descriptor, and HidD_SetOutputReport as a second
route when an interrupt write does not complete.
"""
import ctypes
from ctypes import wintypes as wt

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
hid = ctypes.WinDLL("hid", use_last_error=True)

GENERIC_READ, GENERIC_WRITE = 0x80000000, 0x40000000
FILE_SHARE_READ, FILE_SHARE_WRITE = 1, 2
OPEN_EXISTING = 3
FILE_FLAG_OVERLAPPED = 0x40000000
INVALID_HANDLE_VALUE = wt.HANDLE(-1).value
ERROR_IO_PENDING = 997
ERROR_OPERATION_ABORTED = 995
WAIT_OBJECT_0, WAIT_TIMEOUT = 0, 0x102
HIDP_STATUS_SUCCESS = 0x00110000


class OVERLAPPED(ctypes.Structure):
    _fields_ = [("Internal", ctypes.c_size_t), ("InternalHigh", ctypes.c_size_t),
                ("Offset", wt.DWORD), ("OffsetHigh", wt.DWORD), ("hEvent", wt.HANDLE)]


class HIDD_ATTRIBUTES(ctypes.Structure):
    _fields_ = [("Size", wt.ULONG), ("VendorID", wt.USHORT), ("ProductID", wt.USHORT), ("VersionNumber", wt.USHORT)]


class HIDP_CAPS(ctypes.Structure):
    _fields_ = [("Usage", wt.USHORT), ("UsagePage", wt.USHORT), ("InputReportByteLength", wt.USHORT),
                ("OutputReportByteLength", wt.USHORT), ("FeatureReportByteLength", wt.USHORT),
                ("Reserved", wt.USHORT * 17)] + [(n, wt.USHORT) for n in (
                    "NumberLinkCollectionNodes", "NumberInputButtonCaps", "NumberInputValueCaps",
                    "NumberInputDataIndices", "NumberOutputButtonCaps", "NumberOutputValueCaps",
                    "NumberOutputDataIndices", "NumberFeatureButtonCaps", "NumberFeatureValueCaps",
                    "NumberFeatureDataIndices")]


kernel32.CreateFileW.argtypes = [wt.LPCWSTR, wt.DWORD, wt.DWORD, wt.LPVOID, wt.DWORD, wt.DWORD, wt.HANDLE]
kernel32.CreateFileW.restype = wt.HANDLE
kernel32.CreateEventW.argtypes = [wt.LPVOID, wt.BOOL, wt.BOOL, wt.LPCWSTR]
kernel32.CreateEventW.restype = wt.HANDLE
kernel32.ReadFile.argtypes = [wt.HANDLE, wt.LPVOID, wt.DWORD, ctypes.POINTER(wt.DWORD), ctypes.POINTER(OVERLAPPED)]
kernel32.WriteFile.argtypes = [wt.HANDLE, wt.LPCVOID, wt.DWORD, ctypes.POINTER(wt.DWORD), ctypes.POINTER(OVERLAPPED)]
kernel32.GetOverlappedResult.argtypes = [wt.HANDLE, ctypes.POINTER(OVERLAPPED), ctypes.POINTER(wt.DWORD), wt.BOOL]
kernel32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
kernel32.WaitForSingleObject.restype = wt.DWORD
kernel32.CancelIoEx.argtypes = [wt.HANDLE, ctypes.POINTER(OVERLAPPED)]
kernel32.CloseHandle.argtypes = [wt.HANDLE]
kernel32.ResetEvent.argtypes = [wt.HANDLE]
hid.HidD_GetAttributes.argtypes = [wt.HANDLE, ctypes.POINTER(HIDD_ATTRIBUTES)]
hid.HidD_GetPreparsedData.argtypes = [wt.HANDLE, ctypes.POINTER(ctypes.c_void_p)]
hid.HidD_FreePreparsedData.argtypes = [ctypes.c_void_p]
hid.HidP_GetCaps.argtypes = [ctypes.c_void_p, ctypes.POINTER(HIDP_CAPS)]
hid.HidP_GetCaps.restype = ctypes.c_long
hid.HidD_SetNumInputBuffers.argtypes = [wt.HANDLE, wt.ULONG]
hid.HidD_SetOutputReport.argtypes = [wt.HANDLE, wt.LPVOID, wt.ULONG]


class HidError(OSError):
    pass


def _error(what):
    code = ctypes.get_last_error()
    return HidError(code, f"{what} failed: {ctypes.FormatError(code).strip()}")


class HidCollection:
    """One HID top-level collection opened for overlapped read/write."""

    INPUT_BUFFERS = 512

    def __init__(self, path):
        if isinstance(path, bytes):
            path = path.decode("utf8", "replace")
        self.path = path
        h = kernel32.CreateFileW(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                                 None, OPEN_EXISTING, FILE_FLAG_OVERLAPPED, None)
        if not h or h == INVALID_HANDLE_VALUE:
            raise _error("open")
        self.handle = h
        self.attributes = self._attributes()
        self.caps = self._caps()
        self.input_buffers = self.INPUT_BUFFERS if hid.HidD_SetNumInputBuffers(h, self.INPUT_BUFFERS) else None
        self._read_buf = ctypes.create_string_buffer(max(self.caps.InputReportByteLength, 1))
        self._read_ov = OVERLAPPED(hEvent=kernel32.CreateEventW(None, True, False, None))
        self._read_pending = False
        self._write_ov = OVERLAPPED(hEvent=kernel32.CreateEventW(None, True, False, None))
        self.use_output_report = False  # second route, chosen after a failed write

    def _attributes(self):
        a = HIDD_ATTRIBUTES(Size=ctypes.sizeof(HIDD_ATTRIBUTES))
        if not hid.HidD_GetAttributes(self.handle, ctypes.byref(a)):
            raise _error("HidD_GetAttributes")
        return a

    def _caps(self):
        prep = ctypes.c_void_p()
        if not hid.HidD_GetPreparsedData(self.handle, ctypes.byref(prep)):
            raise _error("HidD_GetPreparsedData")
        try:
            caps = HIDP_CAPS()
            if hid.HidP_GetCaps(prep, ctypes.byref(caps)) != HIDP_STATUS_SUCCESS:
                raise HidError(0, "HidP_GetCaps failed")
            return caps
        finally:
            hid.HidD_FreePreparsedData(prep)

    def read(self, timeout_ms):
        """One input report (without the report ID byte), or None on timeout.

        The read stays pending across timeouts instead of being cancelled, so a
        report that completes later is never lost."""
        n = wt.DWORD(0)
        if not self._read_pending:
            kernel32.ResetEvent(self._read_ov.hEvent)
            if kernel32.ReadFile(self.handle, self._read_buf, len(self._read_buf), None, ctypes.byref(self._read_ov)):
                pass
            elif ctypes.get_last_error() != ERROR_IO_PENDING:
                raise _error("read")
            self._read_pending = True
        if kernel32.WaitForSingleObject(self._read_ov.hEvent, timeout_ms) != WAIT_OBJECT_0:
            return None
        self._read_pending = False
        if not kernel32.GetOverlappedResult(self.handle, ctypes.byref(self._read_ov), ctypes.byref(n), False):
            raise _error("read")
        return self._read_buf.raw[1:n.value] if n.value else b""

    def write(self, report, timeout_ms=250):
        """Write one output report (report ID first). Falls back to
        HidD_SetOutputReport if an interrupt write fails or times out."""
        buf = ctypes.create_string_buffer(bytes(report), self.caps.OutputReportByteLength or len(report))
        if not self.use_output_report:
            n = wt.DWORD(0)
            kernel32.ResetEvent(self._write_ov.hEvent)
            ok = kernel32.WriteFile(self.handle, buf, len(buf), None, ctypes.byref(self._write_ov))
            if ok or ctypes.get_last_error() == ERROR_IO_PENDING:
                if kernel32.WaitForSingleObject(self._write_ov.hEvent, timeout_ms) == WAIT_OBJECT_0:
                    if kernel32.GetOverlappedResult(self.handle, ctypes.byref(self._write_ov), ctypes.byref(n), False):
                        return "write"
                else:
                    kernel32.CancelIoEx(self.handle, ctypes.byref(self._write_ov))
                    kernel32.GetOverlappedResult(self.handle, ctypes.byref(self._write_ov), ctypes.byref(n), True)
            self.use_output_report = True
        if hid.HidD_SetOutputReport(self.handle, buf, len(buf)):
            return "output-report"
        raise _error("write")

    def close(self):
        if self.handle:
            kernel32.CancelIoEx(self.handle, None)
            if self._read_pending:
                n = wt.DWORD(0)
                kernel32.GetOverlappedResult(self.handle, ctypes.byref(self._read_ov), ctypes.byref(n), True)
            kernel32.CloseHandle(self._read_ov.hEvent)
            kernel32.CloseHandle(self._write_ov.hEvent)
            kernel32.CloseHandle(self.handle)
            self.handle = None

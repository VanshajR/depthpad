"""Per-device key events via Windows Raw Input.

Only events from devices whose path contains the AK680's VID/PID are passed on,
so other keyboards (e.g. a laptop's built-in one) never affect the gamepad.
Raw Input observes keys; it does not block them from reaching applications.
"""
import ctypes
import threading
import time
from ctypes import wintypes as wt

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

WM_INPUT = 0x00FF
WM_INPUT_DEVICE_CHANGE = 0x00FE
GIDC_REMOVAL = 2
WM_CLOSE = 0x0010
RID_INPUT = 0x10000003
RIDI_DEVICENAME = 0x20000007
RIM_TYPEKEYBOARD = 1
RIDEV_INPUTSINK = 0x00000100
RIDEV_DEVNOTIFY = 0x00002000
RI_KEY_BREAK = 1
RI_KEY_E0 = 2
HWND_MESSAGE = wt.HWND(-3)

LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM)


class RAWINPUTDEVICE(ctypes.Structure):
    _fields_ = [("usUsagePage", wt.USHORT), ("usUsage", wt.USHORT), ("dwFlags", wt.DWORD), ("hwndTarget", wt.HWND)]


class RAWINPUTHEADER(ctypes.Structure):
    _fields_ = [("dwType", wt.DWORD), ("dwSize", wt.DWORD), ("hDevice", wt.HANDLE), ("wParam", wt.WPARAM)]


class RAWKEYBOARD(ctypes.Structure):
    _fields_ = [("MakeCode", wt.USHORT), ("Flags", wt.USHORT), ("Reserved", wt.USHORT),
                ("VKey", wt.USHORT), ("Message", wt.UINT), ("ExtraInformation", wt.ULONG)]


class RAWINPUT(ctypes.Structure):
    _fields_ = [("header", RAWINPUTHEADER), ("keyboard", RAWKEYBOARD)]


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", wt.UINT), ("lpfnWndProc", WNDPROC), ("cbClsExtra", ctypes.c_int), ("cbWndExtra", ctypes.c_int),
                ("hInstance", wt.HINSTANCE), ("hIcon", wt.HICON), ("hCursor", wt.HANDLE), ("hbrBackground", wt.HBRUSH),
                ("lpszMenuName", wt.LPCWSTR), ("lpszClassName", wt.LPCWSTR)]


user32.DefWindowProcW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user32.DefWindowProcW.restype = LRESULT
user32.CreateWindowExW.argtypes = [wt.DWORD, wt.LPCWSTR, wt.LPCWSTR, wt.DWORD, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                   ctypes.c_int, wt.HWND, wt.HMENU, wt.HINSTANCE, wt.LPVOID]
user32.CreateWindowExW.restype = wt.HWND
user32.GetRawInputData.argtypes = [wt.HANDLE, wt.UINT, wt.LPVOID, ctypes.POINTER(wt.UINT), wt.UINT]
user32.GetRawInputDeviceInfoW.argtypes = [wt.HANDLE, wt.UINT, wt.LPVOID, ctypes.POINTER(wt.UINT)]
user32.RegisterRawInputDevices.argtypes = [ctypes.POINTER(RAWINPUTDEVICE), wt.UINT, wt.UINT]
user32.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user32.GetMessageW.argtypes = [ctypes.POINTER(wt.MSG), wt.HWND, wt.UINT, wt.UINT]
kernel32.GetModuleHandleW.restype = wt.HMODULE

# Scan code set 1 (Raw Input MakeCode, E0 prefix as 0xE000) -> HID keyboard usage.
_PLAIN = {
    0x01: 41, 0x02: 30, 0x03: 31, 0x04: 32, 0x05: 33, 0x06: 34, 0x07: 35, 0x08: 36, 0x09: 37, 0x0A: 38, 0x0B: 39,
    0x0C: 45, 0x0D: 46, 0x0E: 42, 0x0F: 43, 0x10: 20, 0x11: 26, 0x12: 8, 0x13: 21, 0x14: 23, 0x15: 28, 0x16: 24,
    0x17: 12, 0x18: 18, 0x19: 19, 0x1A: 47, 0x1B: 48, 0x1C: 40, 0x1D: 224, 0x1E: 4, 0x1F: 22, 0x20: 7, 0x21: 9,
    0x22: 10, 0x23: 11, 0x24: 13, 0x25: 14, 0x26: 15, 0x27: 51, 0x28: 52, 0x29: 53, 0x2A: 225, 0x2B: 49, 0x2C: 29,
    0x2D: 27, 0x2E: 6, 0x2F: 25, 0x30: 5, 0x31: 17, 0x32: 16, 0x33: 54, 0x34: 55, 0x35: 56, 0x36: 229, 0x38: 226,
    0x39: 44, 0x3A: 57,
}
_E0 = {0x1D: 228, 0x38: 230, 0x5B: 227, 0x5C: 231, 0x47: 74, 0x48: 82, 0x49: 75, 0x4B: 80, 0x4D: 79, 0x4F: 77,
       0x50: 81, 0x51: 78, 0x52: 73, 0x53: 76}
SCAN_TO_HID = {**_PLAIN, **{0xE000 | k: v for k, v in _E0.items()}}


class DeviceKeys(threading.Thread):
    """Calls on_key(hid_usage, down, time.perf_counter()) for keys of the matching
    device, and on_removed() when one of its keyboard interfaces disappears."""

    def __init__(self, on_key, on_removed=lambda: None, device_match="vid_0c45&pid_80bc"):
        super().__init__(daemon=True, name="ak680-rawinput")
        self.on_key = on_key
        self.on_removed = on_removed
        self.device_match = device_match.lower()
        self._names = {}
        self._hwnd = None
        self._ready = threading.Event()
        self.error = None
        self.events = 0

    def _device_name(self, handle):
        name = self._names.get(handle)
        if name is None:
            size = wt.UINT(0)
            user32.GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, None, ctypes.byref(size))
            buf = ctypes.create_unicode_buffer(size.value + 1)
            user32.GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, buf, ctypes.byref(size))
            name = self._names[handle] = buf.value.lower()
        return name

    def _on_input(self, lparam):
        ri = RAWINPUT()
        size = wt.UINT(ctypes.sizeof(ri))
        if user32.GetRawInputData(lparam, RID_INPUT, ctypes.byref(ri), ctypes.byref(size),
                                  ctypes.sizeof(RAWINPUTHEADER)) == 0xFFFFFFFF:
            return
        if ri.header.dwType != RIM_TYPEKEYBOARD or not ri.header.hDevice:
            return
        if self.device_match not in self._device_name(ri.header.hDevice):
            return
        kb = ri.keyboard
        code = kb.MakeCode | (0xE000 if kb.Flags & RI_KEY_E0 else 0)
        usage = SCAN_TO_HID.get(code)
        if usage is None:  # includes the E0 2A / E0 36 fake shifts
            return
        self.events += 1
        self.on_key(usage, not kb.Flags & RI_KEY_BREAK, time.perf_counter())

    def run(self):
        def wndproc(hwnd, msg, wparam, lparam):
            if msg == WM_INPUT:
                self._on_input(lparam)
            elif msg == WM_INPUT_DEVICE_CHANGE and wparam == GIDC_REMOVAL:
                # The name is no longer readable after removal; use the cached one.
                if self.device_match in self._names.pop(lparam, ""):
                    self.on_removed()
            elif msg == WM_CLOSE:
                user32.DestroyWindow(hwnd)
                user32.PostQuitMessage(0)
                return 0
            return user32.DefWindowProcW(hwnd, msg, wparam, lparam)

        self._wndproc = WNDPROC(wndproc)  # keep a reference for the window's lifetime
        hinst = kernel32.GetModuleHandleW(None)
        wc = WNDCLASSW(lpfnWndProc=self._wndproc, hInstance=hinst, lpszClassName="ak680pad_rawinput")
        user32.RegisterClassW(ctypes.byref(wc))
        self._hwnd = user32.CreateWindowExW(0, wc.lpszClassName, "ak680pad", 0, 0, 0, 0, 0, HWND_MESSAGE, None, hinst, None)
        rid = RAWINPUTDEVICE(1, 6, RIDEV_INPUTSINK | RIDEV_DEVNOTIFY, self._hwnd)
        if not self._hwnd or not user32.RegisterRawInputDevices(ctypes.byref(rid), 1, ctypes.sizeof(rid)):
            self.error = ctypes.WinError(ctypes.get_last_error())
            self._ready.set()
            return
        self._ready.set()
        msg = wt.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))

    def start(self):
        super().start()
        self._ready.wait(2)
        if self.error:
            raise self.error

    def stop(self):
        if self._hwnd:
            user32.PostMessageW(self._hwnd, WM_CLOSE, 0, 0)
        self.join(1)

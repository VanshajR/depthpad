"""Optional: keep mapped keys from reaching games, plus a pause/resume hotkey.

A WH_KEYBOARD_LL hook, with the rules of HallJoy's Block Bound Keys
(app.cpp KeyboardBlockHookProc, block_keys_policy.h, AGPL-3.0, PashOK7/HallJoy):
  * only physical input is blocked, never injected keys;
  * the down decides the whole press (repeats and its up follow it), so
    toggling mid-press never swallows an up that should arrive;
  * keys already held when the hook starts pass through;
  * Alt and Tab can be exempted; nothing is blocked while our own console is
    in front; the hotkey press itself never reaches other applications.

Measured on this machine: a key blocked here no longer reaches Raw Input, so
the hook reports blocked presses itself (on_key). The hook cannot tell which
keyboard a key came from, so a blocked key pressed on another keyboard also
counts as the AK680's while blocking is on.
"""
import ctypes
import threading
import time
from ctypes import wintypes as wt

from .rawinput import SCAN_TO_HID

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

WH_KEYBOARD_LL = 13
WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP = 0x100, 0x101, 0x104, 0x105
WM_QUIT = 0x0012
LLKHF_EXTENDED = 0x01
LLKHF_INJECTED = 0x10
MAPVK_VK_TO_VSC_EX = 4
ALT_TAB = {43, 226, 230}   # Tab, Left Alt, Right Alt

LRESULT = ctypes.c_ssize_t
HOOKPROC = ctypes.WINFUNCTYPE(LRESULT, ctypes.c_int, wt.WPARAM, wt.LPARAM)


class KBDLLHOOKSTRUCT(ctypes.Structure):
    _fields_ = [("vkCode", wt.DWORD), ("scanCode", wt.DWORD), ("flags", wt.DWORD),
                ("time", wt.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


user32.CallNextHookEx.argtypes = [wt.HHOOK, ctypes.c_int, wt.WPARAM, wt.LPARAM]
user32.CallNextHookEx.restype = LRESULT
user32.SetWindowsHookExW.argtypes = [ctypes.c_int, HOOKPROC, wt.HINSTANCE, wt.DWORD]
user32.SetWindowsHookExW.restype = wt.HHOOK
user32.GetForegroundWindow.restype = wt.HWND
user32.GetAncestor.argtypes = [wt.HWND, wt.UINT]
user32.GetAncestor.restype = wt.HWND
kernel32.GetConsoleWindow.restype = wt.HWND


def _usage(scan, extended):
    code = scan | (0xE000 if extended else 0)
    return SCAN_TO_HID.get(code)


class KeyBlocker(threading.Thread):
    PASSED, BLOCKED, HOTKEY = 1, 2, 3

    def __init__(self, bound, on_key, on_toggle, block=False, allow_alt_tab=True, hotkey=()):
        """bound: HID usages of mapped keys. hotkey: HID usages, last one triggers."""
        super().__init__(daemon=True, name="ak680-blockhook")
        self.bound = set(bound)
        self.on_key = on_key
        self.on_toggle = on_toggle
        self.block = block
        self.allow_alt_tab = allow_alt_tab
        self.hotkey = tuple(hotkey)
        self.paused = False
        self.routes = {}
        self.held = set()
        self.console = kernel32.GetConsoleWindow()
        self._tid = None
        self._ready = threading.Event()
        self.error = None

    def _own_foreground(self):
        fg = user32.GetForegroundWindow()
        if not fg or not self.console:
            return False
        return fg == self.console or user32.GetAncestor(fg, 2) == self.console

    def _seed(self):
        # Keys already down were delivered to Windows; their release must pass too.
        for vk in range(8, 255):
            if user32.GetAsyncKeyState(vk) & 0x8000:
                sc = user32.MapVirtualKeyW(vk, MAPVK_VK_TO_VSC_EX)
                u = _usage(sc & 0xFF, (sc & 0xFF00) == 0xE000)
                if u:
                    self.routes[u] = self.PASSED
                    self.held.add(u)

    def _hook(self, code, wparam, lparam):
        if code == 0 and wparam in (WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP):
            k = ctypes.cast(lparam, ctypes.POINTER(KBDLLHOOKSTRUCT)).contents
            usage = _usage(k.scanCode & 0xFF, bool(k.flags & LLKHF_EXTENDED))
            if usage and not k.flags & LLKHF_INJECTED:
                down = wparam in (WM_KEYDOWN, WM_SYSKEYDOWN)
                if self._route(usage, down):
                    return 1
        return user32.CallNextHookEx(None, code, wparam, lparam)

    def _route(self, usage, down):
        """True to swallow the event."""
        route = self.routes.get(usage)
        if down:
            if route is None:
                if (self.hotkey and usage == self.hotkey[-1]
                        and all(m in self.held for m in self.hotkey[:-1])):
                    self.paused = not self.paused
                    route = self.HOTKEY
                    self.on_toggle(self.paused)
                elif (self.block and not self.paused and usage in self.bound
                      and not (self.allow_alt_tab and usage in ALT_TAB) and not self._own_foreground()):
                    route = self.BLOCKED
                else:
                    route = self.PASSED
                self.routes[usage] = route
            self.held.add(usage)
        else:
            self.held.discard(usage)
            if route is None:
                return False
            del self.routes[usage]
        if route == self.BLOCKED:
            self.on_key(usage, down, time.perf_counter())
        return route in (self.BLOCKED, self.HOTKEY)

    def run(self):
        self._tid = kernel32.GetCurrentThreadId()
        self._proc = HOOKPROC(self._hook)
        self._seed()
        hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, self._proc, None, 0)
        if not hook:
            self.error = ctypes.WinError(ctypes.get_last_error())
        self._ready.set()
        if not hook:
            return
        msg = wt.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            pass
        user32.UnhookWindowsHookEx(hook)

    def start(self):
        super().start()
        self._ready.wait(2)
        if self.error:
            raise self.error

    def stop(self):
        if self._tid:
            user32.PostThreadMessageW(self._tid, WM_QUIT, 0, 0)
        self.join(1)

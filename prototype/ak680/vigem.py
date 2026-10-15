"""Minimal ViGEmClient binding: one virtual Xbox 360 controller.

Needs the ViGEmBus driver; ViGEmClient.dll (MIT, Nefarius) is in vendor/.
"""
import ctypes
import os

VIGEM_ERROR_NONE = 0x20000000

BUTTONS = {
    "DPAD_UP": 0x0001, "DPAD_DOWN": 0x0002, "DPAD_LEFT": 0x0004, "DPAD_RIGHT": 0x0008,
    "START": 0x0010, "BACK": 0x0020, "LS": 0x0040, "RS": 0x0080, "LB": 0x0100, "RB": 0x0200,
    "GUIDE": 0x0400, "A": 0x1000, "B": 0x2000, "X": 0x4000, "Y": 0x8000,
}


class XUSB_REPORT(ctypes.Structure):
    _fields_ = [("wButtons", ctypes.c_ushort), ("bLeftTrigger", ctypes.c_ubyte), ("bRightTrigger", ctypes.c_ubyte),
                ("sThumbLX", ctypes.c_short), ("sThumbLY", ctypes.c_short),
                ("sThumbRX", ctypes.c_short), ("sThumbRY", ctypes.c_short)]


def _load():
    dll = ctypes.CDLL(os.path.join(os.path.dirname(os.path.abspath(__file__)), "vendor", "ViGEmClient.dll"))
    vp = ctypes.c_void_p
    for name, res, args in [
        ("vigem_alloc", vp, []), ("vigem_free", None, [vp]),
        ("vigem_connect", ctypes.c_uint, [vp]), ("vigem_disconnect", None, [vp]),
        ("vigem_target_x360_alloc", vp, []), ("vigem_target_free", None, [vp]),
        ("vigem_target_add", ctypes.c_uint, [vp, vp]), ("vigem_target_remove", ctypes.c_uint, [vp, vp]),
        ("vigem_target_x360_update", ctypes.c_uint, [vp, vp, XUSB_REPORT]),
    ]:
        f = getattr(dll, name)
        f.restype, f.argtypes = res, args
    return dll


class X360Pad:
    def __init__(self):
        self.dll = _load()
        self.client = self.dll.vigem_alloc()
        err = self.dll.vigem_connect(self.client)
        if err != VIGEM_ERROR_NONE:
            self.dll.vigem_free(self.client)
            raise RuntimeError(f"ViGEmBus not reachable (error 0x{err:08X}) - is the ViGEmBus driver installed?")
        self.target = self.dll.vigem_target_x360_alloc()
        err = self.dll.vigem_target_add(self.client, self.target)
        if err != VIGEM_ERROR_NONE:
            self.close()
            raise RuntimeError(f"Could not plug in the virtual controller (error 0x{err:08X})")
        self.report = XUSB_REPORT()

    def update(self, buttons, lt, rt, lx, ly, rx, ry):
        """buttons: bitmask; triggers 0..1; sticks -1..1."""
        r = XUSB_REPORT(buttons, round(lt * 255), round(rt * 255),
                        *(max(-32768, min(32767, round(v * 32767))) for v in (lx, ly, rx, ry)))
        if bytes(r) != bytes(self.report):
            self.report = r
            self.dll.vigem_target_x360_update(self.client, self.target, r)

    def close(self):
        if self.target:
            self.dll.vigem_target_remove(self.client, self.target)
            self.dll.vigem_target_free(self.target)
            self.target = None
        if self.client:
            self.dll.vigem_disconnect(self.client)
            self.dll.vigem_free(self.client)
            self.client = None

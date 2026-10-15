"""Plug in the virtual pad, set known values, read them back through XInput."""
import ctypes, os, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from ak680.vigem import X360Pad, BUTTONS, XUSB_REPORT

class XINPUT_STATE(ctypes.Structure):
    _fields_ = [("dwPacketNumber", ctypes.c_ulong), ("Gamepad", XUSB_REPORT)]

xi = ctypes.WinDLL("xinput1_4")
before = {i for i in range(4) if xi.XInputGetState(i, ctypes.byref(XINPUT_STATE())) == 0}
pad = X360Pad()
try:
    time.sleep(1.0)
    pad.update(BUTTONS["A"], 0.5, 0.0, 0.0, 1.0, -0.5, 0.0)
    time.sleep(0.2)
    for i in range(4):
        s = XINPUT_STATE()
        if xi.XInputGetState(i, ctypes.byref(s)) == 0 and i not in before:
            g = s.Gamepad
            print(f"slot {i}: buttons={g.wButtons:04X} LT={g.bLeftTrigger} LY={g.sThumbLY} RX={g.sThumbRX}")
            assert g.wButtons == BUTTONS["A"] and g.bLeftTrigger == 128 and g.sThumbLY == 32767 and g.sThumbRX == -16384
            print("ViGEm OK")
            break
    else:
        raise SystemExit("virtual pad not visible through XInput")
finally:
    pad.close()

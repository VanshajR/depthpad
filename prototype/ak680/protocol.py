"""AJAZZ AK680 V2 HE (Sonix, 0C45:80BC) vendor protocol.

Commands are 64-byte reports (plus report ID 0) on the vendor collection
FF68:0061:  AA <cmd> <len> <addr lo> <addr hi> 00 <last> 00 <data...>
Replies start with 55 <cmd> <len> <addr lo> <addr hi>. While
SET_SIMULATION_TEST_ON (0x66) is active the keyboard streams 55 FB packets at ~1 kHz:
  [2] position  [3] calibration status  [4..5] max ADC  [6..7] min ADC
  [8..9] current ADC  [10..11] travel (0.01 mm)  [12..13] full stroke (0.1 mm)

Firmware behaviour measured on 0C45:80BC fw 11.01 (see README): the stream only
repeats the most recently changed key, so it is not a full keyboard state.
"""
import json
import os
import threading
import time

import hid

from .hidio import HidCollection, HidError

VID = 0x0C45
PIDS = (0x80BC,)
USAGE_PAGE = 0xFF68
USAGE = 0x61
REPORT_BYTES = 65

GET_DEVICE_INFO = 0x10
GET_GAME_MODE = 0x11
GET_KEY = 0x12
SIMULATION_ON = 0x66
SIMULATION_OFF = 0x67
STREAM = 0xFB

_HERE = os.path.dirname(os.path.abspath(__file__))


def load_keymap():
    """Position -> {"hid": usage, "key": KeyboardEvent.code} from the AJAZZ configurator."""
    with open(os.path.join(_HERE, "ak680v2_keymap.json"), encoding="utf8") as f:
        return {int(pos): v for pos, v in json.load(f).items()}


def request(cmd, length=0, addr=0, last=1, data=b""):
    r = [0] * 65
    r[1], r[2], r[3], r[4], r[5], r[7] = 0xAA, cmd, length, addr & 0xFF, addr >> 8, last
    r[9:9 + len(data)] = list(data)
    return r


def find_devices():
    return [d for d in hid.enumerate(VID, 0)
            if d["product_id"] in PIDS and d["usage_page"] == USAGE_PAGE and d["usage"] == USAGE]


def parse_stream(packet):
    """Return (position, travel_0.01mm, stroke_0.1mm) or None for anything else.

    About 1.5% of packets are sent while the firmware is clearing its buffer.
    The clearing reaches either the header or the stroke field before it
    reaches travel (a 45 s capture: 653 bad headers, 22 packets with stroke 0
    and zeroed or half-zeroed travel/ADC, no other damage), so a sane stroke is
    required."""
    if len(packet) < 14 or packet[0] != 0x55 or packet[1] != STREAM or packet[2] >= 128:
        return None
    travel = packet[10] | packet[11] << 8
    stroke = packet[12] | packet[13] << 8
    if not 10 <= stroke <= 50 or travel > stroke * 10 + 100:
        return None
    return packet[2], travel, stroke


def keyboard_usage(u):
    return 4 <= u <= 0xA4 or 0xE0 <= u <= 0xE7


def assigned_usage(factory, record):
    """HID usage a physical key currently sends, from its 4-byte GET_KEY record.

    A port of HallJoy's mini60::Assigned with the Driveall rules it uses for the
    AK820 (src/HallJoyProject/HallJoy/aula_mini60_native_model.h): type 0 is the
    factory key; Driveall advanced keys (DKS 8, MT 9, TGL 10, SOCD 11, RS 12) keep
    their own key; type 2 is a keyboard key, p1 = one modifier bit or p2 = usage;
    anything else (macros, media, Fn 0xAF) sends no single keyboard usage -> None."""
    kind, p1, p2, p3 = record
    if not factory or not keyboard_usage(factory):
        return None  # e.g. Fn: handled inside the keyboard, never sent to Windows
    if kind == 0 or 8 <= kind <= 12:
        return factory
    if kind != 2 or p3:
        return None
    if p1:
        if p2 or p1 & (p1 - 1):
            return None
        return 224 + p1.bit_length() - 1
    return p2 if keyboard_usage(p2) else None


def key_layout(keymap, records):
    """Map each physical key to the usage it sends now (remaps applied), and back."""
    pos_to_usage = {}
    for pos, v in keymap.items():
        rec = records[pos] if records and pos < len(records) else (0, 0, 0, 0)
        u = assigned_usage(v["hid"], rec)
        if u:
            pos_to_usage[pos] = u
    usage_to_pos = {}
    for pos, u in sorted(pos_to_usage.items()):
        usage_to_pos.setdefault(u, pos)  # two keys sending one usage: the first owns it
    return pos_to_usage, usage_to_pos


class DeviceError(OSError):
    """A problem the user can act on (shown as-is in the app)."""


class _DeviceLock:
    """A named Windows mutex held while a process owns the keyboard."""

    def __init__(self, name):
        import ctypes
        from ctypes import wintypes as wt
        k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k32.CreateMutexW.restype = wt.HANDLE
        k32.CreateMutexW.argtypes = [wt.LPVOID, wt.BOOL, wt.LPCWSTR]
        k32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
        k32.ReleaseMutex.argtypes = [wt.HANDLE]
        k32.CloseHandle.argtypes = [wt.HANDLE]
        self._k32 = k32
        self.handle = k32.CreateMutexW(None, False, name)
        # WAIT_OBJECT_0 (0) or WAIT_ABANDONED (0x80, previous owner crashed) both mean we own it now.
        self.acquired = bool(self.handle) and k32.WaitForSingleObject(self.handle, 0) in (0, 0x80)

    def release(self):
        if self.handle:
            if self.acquired:
                self._k32.ReleaseMutex(self.handle)
            self._k32.CloseHandle(self.handle)
            self.handle = None


class Keyboard:
    """The vendor collection of one AK680 V2, admitted only after HallJoy's
    checks: one matching device, 65-byte input/output reports, and a device-info
    reply that echoes the same VID/PID."""

    def __init__(self, path=None):
        if path is None:
            devices = find_devices()
            if not devices:
                raise DeviceError("AJAZZ AK680 V2 not found. Connect it with the USB cable.")
            if len(devices) > 1:
                raise DeviceError("More than one AJAZZ AK680 V2 is connected. Connect only one.")
            path = devices[0]["path"]
        # One owner per keyboard across processes (as AnalogSense's Soup does per device family), so the app
        # and the command line never switch the stream off under each other.
        self._mutex = _DeviceLock("Local\\Depthpad-0C45-80BC")
        if not self._mutex.acquired:
            self._mutex.release()
            raise DeviceError("Another Depthpad window or command line is already using the keyboard.")
        try:
            self.dev = HidCollection(path)
        except BaseException:
            self._mutex.release()
            raise
        try:
            caps = self.dev.caps
            if ((caps.UsagePage, caps.Usage) != (USAGE_PAGE, USAGE)
                    or caps.InputReportByteLength != REPORT_BYTES or caps.OutputReportByteLength != REPORT_BYTES):
                raise DeviceError(f"Unexpected HID collection {caps.UsagePage:04X}:{caps.Usage:04X} "
                                  f"in={caps.InputReportByteLength} out={caps.OutputReportByteLength}")
            self.info = self.device_info()
            if not self.info:
                raise DeviceError("The keyboard did not answer the device-info request.")
            if (self.info["vid"], self.info["pid"]) != (self.dev.attributes.VendorID, self.dev.attributes.ProductID):
                raise DeviceError(f"Device-info reply does not match this device "
                                  f"({self.info['vid']:04X}:{self.info['pid']:04X}).")
        except BaseException:
            self.dev.close()
            self._mutex.release()
            raise

    def _await(self, cmd, length, addr, timeout, accept_stream=False):
        """The reply to a request: 55 <cmd> <length> <addr lo> <addr hi>, as HallJoy's Reply()."""
        end = time.perf_counter() + timeout
        while time.perf_counter() < end:
            p = self.dev.read(20)
            if not p or len(p) < 8 or p[0] != 0x55:
                continue
            if accept_stream and p[1] == STREAM:
                return p
            if p[1] == cmd and p[2] == length and (p[3] | p[4] << 8) == addr:
                return p
        return None

    def command(self, cmd, length=0, addr=0, last=1, timeout=0.4):
        self.dev.write(request(cmd, length, addr, last))
        return self._await(cmd, length, addr, timeout)

    def device_info(self):
        r = self.command(GET_DEVICE_INFO, 48)
        if not r:
            return None
        p = r[8:]
        u16 = lambda o: p[o] | p[o + 1] << 8
        return {"vid": u16(4), "pid": u16(6), "version": f"{p[8]:x}.{p[9]:02x}",
                "manufacturer": u16(12), "product": u16(14)}

    def key_records(self):
        """GET_KEY: 128 records of 4 bytes (type, p1, p2, p3), read in 56-byte pieces."""
        data = bytearray()
        for addr in range(0, 512, 56):
            n = min(56, 512 - addr)
            r = self.command(GET_KEY, n, addr, last=1 if addr + n >= 512 else 0)
            if not r:
                return None
            data += r[8:8 + n]
        return [tuple(data[i * 4:i * 4 + 4]) for i in range(128)]

    def start_stream(self):
        """Turn the stream on; True once the keyboard acknowledged or started streaming."""
        self.dev.write(request(SIMULATION_ON))
        return self._await(SIMULATION_ON, 0, 0, 0.3, accept_stream=True) is not None

    def stop_stream(self):
        """Turn the stream off; True if acknowledged within 180 ms (as HallJoy waits)."""
        self.dev.write(request(SIMULATION_OFF))
        return self._await(SIMULATION_OFF, 0, 0, 0.18) is not None

    def read(self, timeout_ms):
        return self.dev.read(timeout_ms)

    def close(self):
        try:
            self.dev.close()
        finally:
            self._mutex.release()


class StreamReader(threading.Thread):
    """Owns the keyboard connection: streams samples to on_sample(position,
    travel, stroke, time) and calls on_lost() whenever the analog data can no
    longer be trusted, so held keys are cleared instead of freezing.

    Follows HallJoy's MINI60 session handling: confirm the stream started,
    re-arm a silent stream, clear analog after 1 s of silence, reconnect after
    an I/O error or unplug, and confirm the stream stopped on exit.
    on_connected(keyboard, records) runs on every (re)connect before streaming."""

    REARM_AFTER = 0.5     # s of silence before re-sending 0x66 (the stream never idles while on)
    REARM_EVERY = 1.5     # s between re-arm attempts
    CLEAR_AFTER = 1.0     # s of silence before held analog values are dropped
    RECONNECT_EVERY = 0.4

    def __init__(self, on_sample, on_lost, on_connected=lambda kb, records: None, log=print):
        super().__init__(daemon=True, name="ak680-stream")
        self.kb = None
        self.on_sample = on_sample
        self.on_lost = on_lost
        self.on_connected = on_connected
        self.log = log
        self.stop_event = threading.Event()
        self.packets = 0
        self.dropped = 0
        self.rearms = 0
        self.connects = 0
        self.state = "searching"
        self.error = None

    def _connect(self):
        try:
            kb = Keyboard()
        except OSError as e:
            if str(e) != str(self.error):
                self.log(str(e))
            self.error = e
            self.state = "searching"
            return False
        self.error = None
        self.kb = kb
        self.connects += 1
        self.on_connected(kb, kb.key_records())
        ok = kb.start_stream()
        self.state = "streaming" if ok else "no-ack"
        note = "" if ok else "; stream start not acknowledged, will re-arm"
        self.log(f"keyboard connected (firmware {kb.info['version']}){note}")
        return True

    def _drop(self, why):
        self.on_lost()
        if self.kb is not None:
            try:
                self.kb.close()
            except OSError:
                pass
        self.kb = None
        self.state = "searching"
        self.log(f"keyboard lost ({why}); analog cleared")

    def run(self):
        last_valid = last_rearm = time.perf_counter()
        cleared = False
        while not self.stop_event.is_set():
            if self.kb is None:
                if not self._connect():
                    self.stop_event.wait(self.RECONNECT_EVERY)
                    continue
                last_valid = last_rearm = time.perf_counter()
                cleared = False
            try:
                p = self.kb.read(20)
            except OSError as e:
                self._drop(e)
                continue
            now = time.perf_counter()
            if p:
                s = parse_stream(p)
                if s:
                    self.packets += 1
                    last_valid = now
                    cleared = False
                    self.state = "streaming"
                    self.on_sample(*s, now)
                    continue
                if p[0] != 0x55:
                    self.dropped += 1
            silent = now - last_valid
            if silent > self.CLEAR_AFTER and not cleared:
                self.on_lost()
                cleared = True
                self.state = "silent"
            if silent > self.REARM_AFTER and now - last_rearm > self.REARM_EVERY:
                self.rearms += 1
                last_rearm = now
                try:
                    self.kb.start_stream()
                except OSError as e:
                    self._drop(e)
        if self.kb is not None:
            try:
                acked = self.kb.stop_stream()
                self.log(f"analog stream off (acknowledged={acked})")
                self.kb.close()
            except OSError:
                pass

    def stop(self):
        self.stop_event.set()
        self.join(2)

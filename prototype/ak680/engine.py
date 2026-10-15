"""The running gamepad: keyboard stream + key events -> fusion -> mapper -> virtual pad.

Used by both the desktop app and the command line. Threads:
  stream    keyboard connection and analog stream (protocol.StreamReader)
  rawinput  this keyboard's key events (rawinput.DeviceKeys)
  hook      optional blocking and the pause hotkey (blocking.KeyBlocker)
  output    ~1 kHz: compute the report and send it to the virtual pad
  logwriter session log to disk, off the realtime path
"""
import collections
import ctypes
import queue
import threading
import time

from .fusion import Fusion
from .mapper import Mapper
from .protocol import StreamReader, key_layout, load_keymap
from .rawinput import DeviceKeys

NEUTRAL = (0, 0.0, 0.0, (0.0, 0.0, 0.0, 0.0))


class SessionLog(threading.Thread):
    """Writes the --log format used by tools/check_log.py from a queue."""

    def __init__(self, path):
        super().__init__(daemon=True, name="ak680-logwriter")
        self.q = queue.SimpleQueue()
        self.f = open(path, "w", encoding="utf8", newline="\n")
        self.t0 = time.perf_counter()
        self.start()

    def put(self, t, text):
        self.q.put((t, text))

    def run(self):
        while True:
            item = self.q.get()
            if item is None:
                break
            t, text = item
            self.f.write(f"{(t - self.t0) * 1000:9.1f} {text}\n")
        self.f.close()

    def close(self):
        self.q.put(None)
        self.join(2)


class Engine:
    def __init__(self, profile, log_path=None, virtual_pad=True):
        self.keymap = load_keymap()
        self.fusion = Fusion()
        self.mapper = Mapper(profile, self.keymap)
        self.profile = profile
        self.events = collections.deque(maxlen=200)
        self.output = NEUTRAL
        self.paused = False
        self.firmware = None
        self.pad = None
        self.pad_error = None
        self.virtual_pad = virtual_pad
        self.blocker = None
        self.blocker_error = None
        self._usage_to_pos, self._pos_to_usage = {}, {}
        self._stop = threading.Event()
        self._log = SessionLog(log_path) if log_path else None
        self._packets_window = collections.deque()
        self._rate = 0.0
        self._rearm_times = collections.deque()
        self._last_rearms = 0
        self.stream = StreamReader(self._on_sample, self._on_lost, self._on_connected, self.event)
        self.keys = DeviceKeys(self._on_raw_key, self._on_removed)
        self._out_thread = threading.Thread(target=self._output_loop, daemon=True, name="ak680-output")

    # --- events from the input threads -------------------------------------------------
    def event(self, text):
        self.events.append((time.time(), text))
        if self._log:
            self._log.put(time.perf_counter(), "E " + text.replace("\n", " "))

    def _on_sample(self, pos, travel, stroke, t):
        if self._log:
            self._log.put(t, f"S {pos} {travel}")
        self.fusion.on_stream(pos, travel, stroke, t)

    def _on_key(self, pos, down, t):
        if self._log:
            self._log.put(t, f"K {pos} {int(down)}")
        self.fusion.on_digital(pos, down, t)

    def _on_raw_key(self, usage, down, t):
        pos = self._usage_to_pos.get(usage)
        if pos is not None:
            self._on_key(pos, down, t)

    def _on_lost(self):
        self.fusion.clear_stream()

    def _on_removed(self):
        self.fusion.release_all()
        self.event("keyboard removed; keys released")

    def _on_connected(self, kb, records):
        self.firmware = kb.info["version"]
        self._pos_to_usage, self._usage_to_pos = key_layout(self.keymap, records)
        if records is None:
            self.event("could not read key assignments; using factory layout")
        else:
            remapped = sum(1 for pos, u in self._pos_to_usage.items() if u != self.keymap[pos]["hid"])
            if remapped:
                self.event(f"{remapped} key(s) remapped on the keyboard; following them")
        self._update_blocker()

    def _on_toggle(self, paused):
        self.paused = paused
        self.event("paused" if paused else "resumed")

    # --- lifecycle -----------------------------------------------------------------------
    def start(self):
        if self.virtual_pad:
            try:
                from .vigem import X360Pad
                self.pad = X360Pad()
            except (OSError, RuntimeError) as e:
                self.pad_error = str(e)
                self.event(f"virtual controller unavailable: {e}")
        self._usage_to_pos = {v["hid"]: p for p, v in self.keymap.items() if v["hid"]}
        self._update_blocker()
        self.keys.start()
        self.stream.start()
        self._out_thread.start()

    def stop(self):
        self._stop.set()
        self._out_thread.join(1)
        self.stream.stop()
        self.keys.stop()
        if self.blocker:
            self.blocker.stop()
        if self.pad:
            self.pad.close()
            self.pad = None
        if self._log:
            self._log.close()

    # --- control from the UI ---------------------------------------------------------------
    def set_profile(self, profile):
        self.mapper = Mapper(profile, self.keymap)
        self.profile = profile
        self._update_blocker()

    def set_paused(self, paused):
        if paused != self.paused:
            if self.blocker:
                self.blocker.paused = paused
            self._on_toggle(paused)

    def _update_blocker(self):
        s = self.profile["settings"]
        bound = {self._pos_to_usage.get(p, self.keymap[p]["hid"]) for p in self.mapper.positions}
        hotkey = [self._pos_to_usage.get(p, self.keymap[p]["hid"]) for p in self.mapper.hotkey]
        if self.blocker is None:
            if not (s.get("block_keys") or hotkey):
                return
            from .blocking import KeyBlocker
            try:
                self.blocker = KeyBlocker(bound, self._on_blocked_key, self._on_toggle, block=s.get("block_keys", False),
                                          allow_alt_tab=s.get("block_allow_alt_tab", True), hotkey=hotkey)
                self.blocker.start()
            except OSError as e:
                self.blocker, self.blocker_error = None, str(e)
                self.event(f"key blocking unavailable: {e}")
            return
        self.blocker.bound = set(bound)
        self.blocker.block = s.get("block_keys", False)
        self.blocker.allow_alt_tab = s.get("block_allow_alt_tab", True)
        self.blocker.hotkey = tuple(hotkey)

    def _on_blocked_key(self, usage, down, t):
        pos = self._usage_to_pos.get(usage)
        if pos is not None:
            self._on_key(pos, down, t)

    # --- realtime output -------------------------------------------------------------------
    def _output_loop(self):
        winmm = ctypes.windll.winmm
        winmm.timeBeginPeriod(1)
        last = None
        try:
            while not self._stop.is_set():
                now = time.perf_counter()
                out = NEUTRAL if self.paused else self.mapper.compute(self.fusion, now)
                b, lt, rt, (lx, ly, rx, ry) = out
                self.output = out
                if self.pad:
                    self.pad.update(b, lt, rt, lx, ly, rx, ry)
                if self._log:
                    o = (b, round(lt, 2), round(rt, 2), round(lx, 2), round(ly, 2), round(rx, 2), round(ry, 2))
                    if o != last:
                        last = o
                        self._log.put(now, "O " + " ".join(map(str, o)))
                time.sleep(0.001)
        finally:
            winmm.timeEndPeriod(1)
            if self.pad:
                self.pad.update(0, 0, 0, 0, 0, 0, 0)

    # --- state for the UI ------------------------------------------------------------------
    def snapshot(self):
        now = time.perf_counter()
        w = self._packets_window
        w.append((now, self.stream.packets))
        while len(w) > 2 and now - w[1][0] >= 1.0:   # keep about the last second
            w.popleft()
        t0, p0 = w[0]
        if now - t0 >= 0.25:
            self._rate = (self.stream.packets - p0) / (now - t0)
        rate = self._rate
        keys = {}
        for pos in self.keymap:
            d = self.fusion.depth(pos, now)
            src = self.fusion.source(pos, now)
            if d > 0 or src:
                keys[pos] = (d, src)
        # Repeated re-arming means something else keeps switching the stream off (e.g. the AJAZZ web driver).
        rearms = self.stream.rearms
        if rearms != self._last_rearms:
            self._rearm_times.append(now)
            self._last_rearms = rearms
        while self._rearm_times and now - self._rearm_times[0] > 15:
            self._rearm_times.popleft()
        contention = len(self._rearm_times) >= 2
        return {
            "contention": contention,
            "state": self.stream.state, "error": str(self.stream.error) if self.stream.error else None,
            "firmware": self.firmware, "rate": rate, "dropped": self.stream.dropped,
            "keys": keys, "output": self.output, "paused": self.paused,
            "blocking": bool(self.profile["settings"].get("block_keys")) and not self.paused and self.blocker is not None,
            "pad": self.pad is not None, "pad_error": self.pad_error,
        }

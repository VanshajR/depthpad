"""Combine the single-focus analog stream with the keyboard's own key events.

Measured on 0C45:80BC (see README):
  * the stream repeats only the most recently changed key at ~1 kHz;
  * a key that loses focus is never reported again, so its release is lost;
  * a key pressed while another key keeps focus may never be reported at all.

Rules per key:
  * stream packets for the key within FRESH seconds -> the stream is exact, use it;
  * otherwise, key down per the keyboard's HID reports -> keep the last streamed
    depth (or full travel if the stream never reported this press);
  * key-up of a key the stream was not reporting -> the key is at the keyboard's
    release point at that moment (learned from streamed keys, whose key-up
    arrives at a consistent depth), so start from there and ease to 0 over
    RELEASE_RAMP instead of jumping to 0;
  * otherwise -> released.
"""
import threading

FRESH = 0.030           # s; a focused key is repeated every ~1 ms, two moving keys interleave every ~2 ms
RELEASE_RAMP = 0.100    # s; estimated time from the release point to fully up for an unstreamed key
LEARN_WINDOW = 0.005    # s; a stream packet this close to a key-up shows the key's release depth
DEFAULT_STROKE = 34     # 0.1 mm; the AK680 V2 reports 3.4 mm
DEFAULT_RELEASE = 145   # 0.01 mm; measured key-up depth with the keyboard's default settings


class KeyState:
    __slots__ = ("travel", "stream_time", "down", "stroke", "release", "ramp_from", "ramp_time")

    def __init__(self):
        self.travel = 0                     # last streamed depth, 0.01 mm
        self.stream_time = float("-inf")    # time.perf_counter() of the last stream packet for this key
        self.down = False                   # digital state from the keyboard's HID reports
        self.stroke = DEFAULT_STROKE
        self.release = None                 # learned key-up depth of this key, 0.01 mm
        self.ramp_from = 0                  # depth at an unstreamed key-up, 0.01 mm
        self.ramp_time = float("-inf")


class Fusion:
    def __init__(self, fresh=FRESH, release_ramp=RELEASE_RAMP):
        self.fresh = fresh
        self.release_ramp = release_ramp
        self.release = DEFAULT_RELEASE      # learned key-up depth across all keys
        self.keys = {}
        self.focus = None
        self._lock = threading.Lock()

    def _key(self, pos):
        k = self.keys.get(pos)
        if k is None:
            k = self.keys[pos] = KeyState()
        return k

    def on_stream(self, pos, travel, stroke, now):
        with self._lock:
            k = self._key(pos)
            k.travel = travel
            k.stream_time = now
            if 10 <= stroke <= 50:
                k.stroke = stroke
            self.focus = pos

    def on_digital(self, pos, down, now):
        with self._lock:
            k = self._key(pos)
            was_down, k.down = k.down, down
            if down or not was_down:
                return
            if now - k.stream_time <= LEARN_WINDOW and 20 < k.travel < k.stroke * 10 - 20:
                # The stream is following this release: learn where the keyboard reports key-up.
                k.release = k.travel if k.release is None else round(0.7 * k.release + 0.3 * k.travel)
                self.release = round(0.8 * self.release + 0.2 * k.travel)
            # Key-up means the key is at its release point now. If the stream keeps
            # reporting it, the stream wins; if not, ease down from there.
            point = k.release or self.release
            k.ramp_from = min(point, k.travel) if k.travel else point
            k.ramp_time = now
            if now - k.stream_time > self.fresh:
                k.travel = 0

    def clear_stream(self):
        """Forget all streamed depths (stream silent or keyboard lost)."""
        with self._lock:
            for k in self.keys.values():
                k.travel = 0
                k.stream_time = float("-inf")
                k.ramp_time = float("-inf")

    def release_all(self):
        """Forget all key-down state (the keyboard was unplugged)."""
        with self._lock:
            for k in self.keys.values():
                k.down = False
                k.travel = 0
                k.ramp_time = float("-inf")

    def depth(self, pos, now):
        """Key depth as a fraction of full travel, 0..1."""
        with self._lock:
            k = self.keys.get(pos)
            if k is None:
                return 0.0
            full = k.stroke * 10
            ramp = None
            if not k.down and now - k.ramp_time < self.release_ramp:
                ramp = k.ramp_from * (1 - (now - k.ramp_time) / self.release_ramp)
            if now - k.stream_time <= self.fresh:
                # After a key-up the key is at most at its release point, even if the
                # stream's last (possibly stale) packet for it says deeper.
                travel = k.travel if ramp is None else min(k.travel, ramp)
            elif k.down:
                travel = k.travel or full
            else:
                travel = ramp or 0
            return min(1.0, travel / full)

    def source(self, pos, now):
        """Which input decided the key's depth: 'stream', 'held', 'release', or None."""
        with self._lock:
            k = self.keys.get(pos)
            if k is None:
                return None
            if now - k.stream_time <= self.fresh:
                return "stream"
            if k.down:
                return "held"
            return "release" if now - k.ramp_time < self.release_ramp else None

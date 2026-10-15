"""Replay a real capture (stream + ground-truth key events) through Fusion.

capture_chords.log was recorded on an AK680 V2 (0C45:80BC, fw 11.01) with
GetAsyncKeyState as ground truth. Events before 20 s came from the laptop's own
keyboard (GetAsyncKeyState is not per-device) and are skipped.

This log predates stroke logging, so the firmware's zeroed glitch packets
(protocol.parse_stream drops them by stroke=0 on the live device) are dropped
here as a lone adc=0/travel=0 right after a deep reading: a real release passes
through intermediate depths with a non-zero ADC first.
"""
import os, re, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from ak680.fusion import Fusion

POS = {"W": 34, "A": 49, "S": 50, "D": 51, "SPACE": 83, "SHIFT": 64}
LOG = os.path.join(os.path.dirname(__file__), "data", "capture_chords.log")


def replay(checkpoints):
    f = Fusion()
    out = {}
    last = {}
    pending = sorted(checkpoints)
    for line in open(LOG):
        m = re.match(r"\s*([\d.]+) (?:DIGITAL (\w+) (down|up)|key=\s*(\d+) st=\d+ adc=(\d+) travel=(\d+))", line)
        if not m:
            continue
        t = float(m.group(1)) / 1000
        while pending and pending[0] <= t:
            c = pending.pop(0)
            out[c] = {name: round(f.depth(p, c), 2) for name, p in POS.items()}
        if t < 20:
            continue
        if m.group(2):
            f.on_digital(POS[m.group(2)], m.group(3) == "down", t)
        else:
            pos, adc, travel = int(m.group(4)), int(m.group(5)), int(m.group(6))
            if adc == 0 and travel == 0 and last.get(pos, 0) > 50:
                continue
            last[pos] = travel
            f.on_stream(pos, travel, 34, t)
    return out


def test_chords():
    r = replay([26.0, 28.5, 30.5, 32.5, 34.0, 36.0, 37.5, 40.0, 42.0, 46.0, 49.0])
    for t, v in sorted(r.items()):
        print(f"{t:5.1f}s", " ".join(f"{k}={x:.2f}" for k, x in v.items()))
    # W held alone, then W+D both held (D took the stream focus)
    assert r[26.0]["W"] > 0.5
    assert r[28.5]["W"] > 0.5 and r[28.5]["D"] > 0.9
    # W released while D held: the stream never sent this release
    assert r[30.5]["W"] == 0 and r[30.5]["D"] > 0.9
    assert r[32.5]["D"] == 0
    # A held, S tapped: the stream never reported S at all
    assert r[34.0]["A"] > 0.9 and r[34.0]["S"] == 0
    assert r[36.0]["S"] == 0
    # W + Space together
    assert r[46.0]["W"] > 0.5 and r[46.0]["SPACE"] > 0.5
    assert r[49.0]["W"] == 0 and r[49.0]["SPACE"] == 0


def test_s_taps_register():
    # S down at 35.163-35.289, 37.440-37.560, 39.694-39.778
    r = replay([35.2, 37.5, 39.75])
    assert all(v["S"] == 1.0 and v["A"] > 0.9 for v in r.values()), r


if __name__ == "__main__":
    test_chords(); test_s_taps_register(); print("replay OK")

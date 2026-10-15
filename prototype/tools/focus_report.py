"""From session logs: when key Y is pressed while key X is held (and X has the
stream), does the stream switch to Y? Reported per (held, new) pair.

Usage: python tools/focus_report.py run1.log [run2.log ...]
"""
import os, sys
from collections import defaultdict
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from ak680.protocol import load_keymap

name = {p: v["key"] for p, v in load_keymap().items()}
result = defaultdict(lambda: [0, 0])  # (held, new) -> [switched, not switched]
for path in sys.argv[1:]:
    events = []
    for line in open(path, encoding="utf8", errors="replace"):
        f = line.split()
        if len(f) == 4 and f[1] in "SK":
            try:
                events.append((float(f[0]), f[1], int(f[2]), int(f[3])))
            except ValueError:
                pass
    down, focus, last_travel = set(), None, {}
    for i, (t, kind, pos, val) in enumerate(events):
        if kind == "S":
            focus = pos
            last_travel[pos] = val
            continue
        if not val:
            down.discard(pos)
            continue
        if pos in down:
            continue  # auto-repeat
        down.add(pos)
        held = focus
        if held is None or held == pos or held not in down or last_travel.get(held, 0) < 100:
            continue
        switched = any(k == "S" and p == pos for tt, k, p, v in events[i:] if tt - t <= 80)
        result[(held, pos)][0 if switched else 1] += 1

print(f"{'held':>12} -> {'pressed':<12} switched  stayed")
for (h, n), (yes, no) in sorted(result.items(), key=lambda x: -sum(x[1])):
    print(f"{name.get(h, h):>12} -> {name.get(n, n):<12} {yes:8d} {no:7d}   (pos {h} -> {n})")

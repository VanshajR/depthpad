"""Re-run a recorded session's inputs (S and K lines) through the current code.

Writes a log with freshly computed controller output (O lines), so a session
recorded with an older version can be checked against changed logic:

  python tools/replay_log.py session.log replayed.log
  python tools/check_log.py replayed.log
"""
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, ROOT)
from ak680.fusion import Fusion
from ak680.protocol import load_keymap
from ak680.mapper import Mapper
from ak680.profile import ProfileStore


def replay(src, dst, config=os.path.join(ROOT, "config.json"), step_ms=1.0):
    mapper = Mapper(ProfileStore.load_file(config), load_keymap())
    fusion = Fusion()
    last, next_tick = None, 0.0
    with open(dst, "w", encoding="utf8") as out:
        def emit(t):
            nonlocal last
            b, lt, rt, ax = mapper.compute(fusion, t / 1000)
            o = (b, round(lt, 2), round(rt, 2), *(round(a, 2) for a in ax))
            if o != last:
                last = o
                out.write(f"{t:9.1f} O {' '.join(map(str, o))}\n")

        for line in open(src, encoding="utf8", errors="replace"):
            f = line.split()
            try:
                t, kind = float(f[0]), f[1]
                args = [int(x) for x in f[2:4]]
            except (ValueError, IndexError):
                continue
            if kind not in "SK" or len(f) != 4:
                continue
            while next_tick < t:  # the live loop runs about every 1 ms
                emit(next_tick)
                next_tick += step_ms
            out.write(line)
            if kind == "S":
                fusion.on_stream(args[0], args[1], 34, t / 1000)
            else:
                fusion.on_digital(args[0], bool(args[1]), t / 1000)
        emit(next_tick)


if __name__ == "__main__":
    replay(sys.argv[1], sys.argv[2])

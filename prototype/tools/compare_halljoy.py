"""Run recorded sessions through HallJoy's two key-state rules for the Sonix
0C45 family and through ours, then score each with check_log.

HallJoy rules, from src/HallJoyProject/HallJoy/aula_mini60_native_model.h:
  mini60 (Read): a key's last streamed travel counts for 50 ms, then 0
  ak820  (Held): a key keeps its last streamed travel until a reported 0 or a
                 value inside the top dead zone (releaseFloor; 0 on this keyboard)
Neither uses key events. Same curve/mapping for all three, so only the
key-state logic differs.

Usage: python tools/compare_halljoy.py session1.log [session2.log ...]
"""
import io, json, os, sys, contextlib
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, ROOT)
from ak680.fusion import Fusion
from ak680.protocol import load_keymap
from ak680.mapper import Mapper
from ak680.profile import ProfileStore


class HallJoyMini60:
    def __init__(self): self.k = {}
    def on_stream(self, pos, travel, stroke, t): self.k[pos] = (travel, t)
    def on_digital(self, *a): pass
    def depth(self, pos, now):
        travel, t = self.k.get(pos, (0, -1e9))
        return min(1.0, travel / 340) if now - t <= 0.050 else 0.0


class HallJoyAk820:
    floor = 0
    def __init__(self): self.k = {}
    def on_stream(self, pos, travel, stroke, t): self.k[pos] = travel
    def on_digital(self, *a): pass
    def depth(self, pos, now):
        travel = self.k.get(pos, 0)
        return 0.0 if travel <= self.floor else min(1.0, travel / 340)


def run(path, model, config):
    mapper = Mapper(ProfileStore.load_file(config), load_keymap())
    out, last, tick = [], None, 0.0
    def emit(t):
        nonlocal last
        b, lt, rt, ax = mapper.compute(model, t / 1000)
        o = (b, round(lt, 2), round(rt, 2), *(round(a, 2) for a in ax))
        if o != last:
            last = o
            out.append(f"{t:9.1f} O {' '.join(map(str, o))}\n")
    for line in open(path, encoding="utf8", errors="replace"):
        f = line.split()
        if len(f) != 4 or f[1] not in "SK":
            continue
        try:
            t, a, b = float(f[0]), int(f[2]), int(f[3])
        except ValueError:
            continue
        while tick < t:
            emit(tick); tick += 1.0
        out.append(line)
        if f[1] == "S":
            model.on_stream(a, b, 34, t / 1000)
        else:
            model.on_digital(a, bool(b), t / 1000)
    return out


if __name__ == "__main__":
    import check_log
    for path in sys.argv[1:]:
        config = os.path.join(ROOT, "config_racing.json" if "racing" in path else "config.json")
        print(f"== {os.path.basename(path)}")
        for name, model in (("HallJoy MINI60 rule", HallJoyMini60()), ("HallJoy AK820 rule", HallJoyAk820()),
                            ("ak680pad", Fusion())):
            tmp = os.path.join(os.environ.get("TEMP", "."), "cmp.log")
            open(tmp, "w", encoding="utf8").writelines(run(path, model, config))
            buf = io.StringIO()
            sys.argv = ["check_log", tmp, "--config", config]
            with contextlib.redirect_stdout(buf):
                check_log.main()
            r = {l.split(":")[0]: l.split(":")[1].split()[0] for l in buf.getvalue().splitlines()
                 if l.split(":")[0] in ("stuck", "missing", "jump")}
            print(f"   {name:20s} stuck {r['stuck']:>4}   missing {r['missing']:>4}   sudden drops {r['jump']:>4}")

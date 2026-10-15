"""Check a `ak680pad.py --log` session for stuck, missing or jumping output.

Ground truth is the keyboard's own key-down/up events (K lines); the checked
value is what was sent to the virtual controller (O lines).

  stuck   : every key of the stick is up (and the stream shows none of them
            partly pressed), yet the stick is still deflected after a grace period
  missing : exactly one key of the stick has been held past the grace period,
            yet the stick does not point that way (every key of that stick
            unchanged for the grace period, so an easing release is not counted)
  jump    : a trigger or stick axis falls by more than 0.6 in a single update
            with no key pressed just before it (pressing the opposite key of
            a stick legitimately cancels it at once).
            The stream reports a releasing key every ~1 ms, and even a fast
            finger takes >10 ms to lift, so a real release never does this.

Usage: python tools/check_log.py run.log [--config config.json]
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from ak680.protocol import load_keymap
from ak680.profile import ProfileStore

GRACE_MS = 150      # key event -> controller output allowance (includes the 100 ms release ramp)
JUMP = 0.6
PARTIAL = 20        # 0.01 mm; a streamed key above this is legitimately partly pressed
STREAM_MS = 30


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log")
    ap.add_argument("--config", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "config.json"))
    args = ap.parse_args()

    by_name = {v["key"]: p for p, v in load_keymap().items()}
    profile = ProfileStore.load_file(args.config)
    # stick -> {pos: (axis index in the O line, sign)}
    sticks = {}
    axis = {"LS_RIGHT": ("left_stick", 3, 1), "LS_LEFT": ("left_stick", 3, -1),
            "LS_UP": ("left_stick", 4, 1), "LS_DOWN": ("left_stick", 4, -1),
            "RS_RIGHT": ("right_stick", 5, 1), "RS_LEFT": ("right_stick", 5, -1),
            "RS_UP": ("right_stick", 6, 1), "RS_DOWN": ("right_stick", 6, -1)}
    for b in profile["bindings"]:
        if b["target"] in axis and b["key"] in by_name:
            name, i, sign = axis[b["target"]]
            sticks.setdefault(name, {})[by_name[b["key"]]] = (i, sign)

    down, since, stream = {}, {}, {}
    out = [0.0] * 7
    issues = {"stuck": [], "missing": [], "jump": []}
    names = ["buttons", "LT", "RT", "LX", "LY", "RX", "RY"]
    checks = 0
    presses = 0
    last_check = {}

    def check(t):
        nonlocal checks
        for name, keys in sticks.items():
            held = [p for p in keys if down.get(p)]
            partial = any(t - stream.get(p, (-1e9, 0))[0] <= STREAM_MS and stream[p][1] > PARTIAL for p in keys)
            if not held and not partial:
                last_up = max((since.get(p, -1e9) for p in keys), default=-1e9)
                if t - last_up > GRACE_MS:
                    checks += 1
                    axes = [out[keys[p][0]] for p in keys]
                    if any(abs(a) > 0.05 for a in axes):
                        issues["stuck"].append((t, name, out[3:7]))
            elif len(held) == 1 and t - max(since.get(p, -1e9) for p in keys) > GRACE_MS:
                checks += 1
                axis, sign = keys[held[0]]
                if out[axis] * sign <= 0.05:
                    issues["missing"].append((t, name, load_keymap()[held[0]]["key"], out[3:7]))

    damaged = 0
    last_press = -1e9
    for line in open(args.log, encoding="utf8", errors="replace"):
        f = line.split()
        try:
            t, kind = float(f[0]), f[1]
            if kind == "E":
                continue
            if kind not in "SKO" or len(f) != {"S": 4, "K": 4, "O": 9}[kind]:
                raise ValueError
            [float(x) for x in f[2:]]
        except (ValueError, IndexError, KeyError):
            damaged += 1
            continue
        if kind == "K":
            p, d = int(f[2]), f[3] == "1"
            if d and not down.get(p):
                last_press = t
                presses += 1
            if down.get(p) != d:
                since[p] = t
            down[p] = d
        elif kind == "S":
            stream[int(f[2])] = (t, int(f[3]))
        elif kind == "O":
            new = [float(x) for x in f[2:9]]
            for i in range(1, 7):
                if abs(out[i]) - abs(new[i]) > JUMP and t - last_press > GRACE_MS:
                    issues["jump"].append((t, names[i], f"{out[i]:+.2f} -> {new[i]:+.2f}"))
            out = new
            check(t)
            last_check["t"] = t
        # also check periodically between outputs (output only changes are logged)
        if kind != "O" and t - last_check.get("t", 0) > 20:
            check(t)
            last_check["t"] = t

    print(f"key presses: {presses}   checks: {checks}   damaged log lines skipped: {damaged}")
    for kind, lst in issues.items():
        # collapse consecutive reports into episodes
        episodes = []
        for item in lst:
            if episodes and item[0] - episodes[-1][1] < 100:
                episodes[-1][1] = item[0]
            else:
                episodes.append([item[0], item[0], item])
        print(f"{kind}: {len(episodes)} episode(s)")
        for start, end, item in episodes[:10]:
            print(f"   {start / 1000:8.2f}s .. {end / 1000:8.2f}s  {item[1:]}")
    if not any(issues.values()):
        print("PASS: the sticks matched your key presses for the whole session")


if __name__ == "__main__":
    main()

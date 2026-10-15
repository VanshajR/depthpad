"""AJAZZ AK680 V2 HE -> virtual Xbox 360 controller (command line).

  python depthpad_cli.py                       run with config.json (Ctrl+C to quit)
  python depthpad_cli.py --config racing.json  another profile or legacy config
  python depthpad_cli.py --monitor             key depths only, no virtual controller
  python depthpad_cli.py --log run.log         record samples, key events and output (tools/check_log.py)
  python depthpad_cli.py --seconds 60          stop on its own
"""
import argparse
import os
import sys
import time

from ak680.engine import Engine
from ak680.profile import ProfileStore, validate
from ak680.protocol import load_keymap


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--config", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "config.json"))
    ap.add_argument("--monitor", action="store_true", help="show key depths only, no virtual controller")
    ap.add_argument("--log", help="record stream samples, key events and controller output to this file")
    ap.add_argument("--seconds", type=float, default=0, help="stop on its own after this many seconds")
    args = ap.parse_args()

    keymap = load_keymap()
    profile = ProfileStore.load_file(args.config)
    problems = validate(profile, {v["key"] for v in keymap.values()})
    if problems:
        sys.exit("config: " + "; ".join(problems))

    engine = Engine(profile, log_path=args.log, virtual_pad=not args.monitor)
    shown = 0

    def notices():
        nonlocal shown
        ev = list(engine.events)
        for _, text in ev[shown:]:
            sys.stdout.write("\r" + text.ljust(150) + "\n")
        shown = len(ev)

    engine.start()
    hotkey = profile["settings"].get("pause_hotkey") or "none"
    print(f"Profile {profile['name']!r}. Pause/resume: {hotkey}. "
          + ("Mapped keys are blocked from other apps. " if profile["settings"].get("block_keys") else "")
          + "Ctrl+C to quit.")
    end = time.perf_counter() + args.seconds if args.seconds else float("inf")
    try:
        while time.perf_counter() < end:
            notices()
            s = engine.snapshot()
            b, lt, rt, (lx, ly, rx, ry) = s["output"]
            active = " ".join(f"{keymap[p]['key']}:{d:.2f}{'*' if src == 'stream' else ''}"
                              for p, (d, src) in sorted(s["keys"].items()) if d > 0)
            line = (f"L({lx:+.2f},{ly:+.2f}) R({rx:+.2f},{ry:+.2f}) LT{lt:.2f} RT{rt:.2f} btn={b:04X} | {active or '-'}"
                    f"  [{'PAUSED ' if s['paused'] else ''}{s['state']} {s['rate']:.0f}/s]")
            sys.stdout.write("\r" + line[:150].ljust(150))
            sys.stdout.flush()
            time.sleep(0.1)
        print()
    except KeyboardInterrupt:
        print()
    finally:
        engine.stop()
        notices()
        print("Stopped; analog stream off, controller removed.")


if __name__ == "__main__":
    main()

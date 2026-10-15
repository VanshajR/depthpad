"""KeyBlocker routing rules (no hook installed; events fed directly)."""
import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from ak680.blocking import KeyBlocker

W, TAB, RCTRL, P, X = 26, 43, 228, 19, 27


def make(block=True):
    seen, toggles = [], []
    b = KeyBlocker({W, TAB}, lambda u, d, t: seen.append((u, d)), toggles.append, block=block, hotkey=(RCTRL, P))
    b._own_foreground = lambda: False
    return b, seen, toggles


def test_bound_key_blocked_and_reported():
    b, seen, _ = make()
    assert b._route(W, True) and b._route(W, True) and b._route(W, False)   # down, repeat, up all swallowed
    assert seen == [(W, True), (W, True), (W, False)]
    assert not b._route(X, True) and not b._route(X, False)                 # unbound passes


def test_alt_tab_exempt():
    b, seen, _ = make()
    assert not b._route(TAB, True) and not b._route(TAB, False) and not seen


def test_down_decides_whole_press():
    b, seen, toggles = make()
    b._route(W, True)                       # blocked press in progress
    b._route(RCTRL, True); assert b._route(P, True)   # hotkey: pause, swallowed
    assert toggles == [True]
    assert b._route(W, False) and seen[-1] == (W, False)   # its up still follows the block route
    assert b._route(P, False)               # hotkey's up swallowed too
    assert not b._route(W, True)            # paused: new presses pass
    b._route(W, False)


def test_up_without_route_passes():
    b, _, _ = make()
    assert not b._route(W, False)


def test_block_off_only_hotkey():
    b, seen, toggles = make(block=False)
    assert not b._route(W, True) and not b._route(W, False) and not seen
    b._route(RCTRL, True); assert b._route(P, True) and toggles == [True]


if __name__ == "__main__":
    for f in [test_bound_key_blocked_and_reported, test_alt_tab_exempt, test_down_decides_whole_press,
              test_up_without_route_passes, test_block_off_only_hotkey]:
        f()
    print("blocking OK")

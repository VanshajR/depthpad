"""AxisResolver must behave like HallJoy's ResolveAxis (configured_xusb_builder.cpp)."""
import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from ak680.sticks import AxisResolver


def test_default_is_difference():
    r = AxisResolver()
    assert r.resolve(0.0, 0.7) == 0.7
    assert abs(r.resolve(0.3, 0.7) - 0.4) < 1e-9


def test_last_key_priority():
    r = AxisResolver(last_key_priority=True)
    assert r.resolve(0.8, 0.0) == -0.8          # left held
    assert r.resolve(0.8, 0.6) == 0.6           # right pressed later wins with its own depth
    assert r.resolve(0.8, 0.0) == -0.8          # right released: left again
    r = AxisResolver(last_key_priority=True)
    r.resolve(1.0, 0.0); r.resolve(1.0, 1.0)    # right wins
    r.resolve(0.5, 1.0)                         # left eased up (valley 0.5)...
    assert r.resolve(0.6, 1.0) == -0.6          # ...and pushed 0.1 deeper: retakes priority


def test_snappy():
    r = AxisResolver(snappy=True)
    assert r.resolve(0.3, 0.7) == 0.7           # deeper side, full value
    r = AxisResolver(snappy=True)
    r.resolve(0.0, 0.5)
    assert r.resolve(0.5, 0.5) == -0.5          # equal -> the most recently pressed side (left)


if __name__ == "__main__":
    test_default_is_difference(); test_last_key_priority(); test_snappy(); print("sticks OK")

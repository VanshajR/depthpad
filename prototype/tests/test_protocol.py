"""Protocol parsing with malformed fixtures, and the key-assignment rules."""
import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from ak680.protocol import parse_stream, assigned_usage, key_layout, load_keymap


def pkt(hexstr):
    b = bytes.fromhex(hexstr)
    return b + bytes(64 - len(b))


GOOD = "55 fb 31 01 eb 09 78 05 26 06 61 01 22 00"          # A, travel 0x161=353, stroke 34


def test_good_packet():
    assert parse_stream(pkt(GOOD)) == (49, 353, 34)


def test_malformed_fixtures_from_captures():
    bad = [
        "00 00 31 01 eb 09 78 85 28 06 60 01 22 00",          # header cleared
        "00 fb 31 01 eb 09 78 85 27 06 60 01 22 00",          # first byte cleared
        "00 00 00 00 00 00 00 00 26 06 61 01 22 00",          # first 8 bytes cleared
        "55 fb 24 00 00 00 00 00 00 00 00 00 00 00",          # header kept, rest cleared (stroke 0)
        "55 fb 22 01 f3 09 78 05 4f 06 5a 00 00 00",          # travel half-cleared (90), stroke 0
        "55 fb 44 01 fc 09 78 00 00 00 00 00 00 00",          # min ADC partly cleared, stroke 0
        "55 fb 90 01 eb 09 78 05 26 06 61 01 22 00",          # position >= 128
        "55 fb 31 01 eb 09 78 05 26 06 ff 7f 22 00",          # travel beyond any possible stroke
        "55 10 30 00 00 00 01 00 00 00 00 92 45 0c bc 80",    # a command reply, not stream
    ]
    for b in bad:
        assert parse_stream(pkt(b)) is None, b
    assert parse_stream(b"\x55\xfb\x31") is None            # short read


def test_assigned_usage():
    W = 26
    assert assigned_usage(W, (0, 0, 0, 0)) == W              # factory
    assert assigned_usage(W, (11, 3, 50, 0)) == W            # SOCD advanced key keeps its own key
    assert assigned_usage(W, (2, 0, 4, 0)) == 4              # remapped to A
    assert assigned_usage(W, (2, 0x02, 0, 0)) == 225         # remapped to Left Shift (modifier bit 1)
    assert assigned_usage(W, (2, 0x03, 0, 0)) is None        # two modifiers: no single key
    assert assigned_usage(W, (2, 0, 0xAF, 0)) is None        # Fn
    assert assigned_usage(W, (3, 0, 0, 0)) is None           # macro/media/other
    assert assigned_usage(175, (0, 0, 0, 0)) is None         # Fn's factory "usage"


def test_key_layout_follows_remaps():
    km = load_keymap()
    rec = [(0, 0, 0, 0)] * 128
    p2u, u2p = key_layout(km, rec)
    assert len(p2u) == 67 and u2p[26] == 34                  # 67 keys send a usage; W is position 34
    rec = list(rec); rec[48] = (2, 0, 86, 0)                 # Caps remapped to keypad minus
    p2u, u2p = key_layout(km, rec)
    assert p2u[48] == 86 and u2p[86] == 48 and 57 not in u2p


if __name__ == "__main__":
    test_good_packet(); test_malformed_fixtures_from_captures(); test_assigned_usage(); test_key_layout_follows_remaps()
    print("protocol OK")

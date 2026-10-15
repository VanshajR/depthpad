# AK680 V2 HE protocol notes

These measurements were taken on an AJAZZ AK680 V2 HE (`0C45:80BC`, firmware 11.01). The keyboard's own key-down/up events were recorded alongside as ground truth. They are also posted as [HallJoy #9](https://github.com/PashOK7/HallJoy/issues/9).

## Interface

The keyboard is a Sonix chip with 4 interfaces. Analog data comes from the vendor collection on interface 2:

- usage page `FF68`, usage `0061`;
- 65-byte input and output reports (report ID 0 + 64 bytes).

The same chip family (manufacturer `0x0166`, product `0x110C`) is used by the AULA MINI60 HE boards and the AK820 MAX HE `0C45:80B1`.

**Request:** `AA <cmd> <len> <addr lo> <addr hi> 00 <last> 00 <data…>`
**Reply:** `55 <cmd> <len> <addr lo> <addr hi> …`. The reply echoes the request's command, length and address.

| Cmd | Name | Use |
|---|---|---|
| `10` | GET_DEVICE_INFO | 48 bytes; VID/PID at +4/+6 of the data, version at +8/+9 |
| `11` | GET_GAME_MODE | dead zones |
| `12` | GET_KEY | 128 × 4-byte key records (type, p1, p2, p3), read in 56-byte pieces |
| `66` / `67` | Simulation test on / off | the analog stream; both are acknowledged |
| `60`, `68` | Key status | not implemented: the keyboard only echoes the request |

## The stream

With `66` on, the keyboard sends `55 FB` packets at about 1 kHz:

| Byte | Field |
|---|---|
| 2 | key position (row×16+col style index; see `ak680/ak680v2_keymap.json`) |
| 3 | calibration status |
| 4–5 / 6–7 | max / min ADC |
| 8–9 | current ADC |
| 10–11 | travel in 0.01 mm |
| 12–13 | full stroke in 0.1 mm (34 = 3.4 mm) |

## How it behaves

1. **It follows one key.** It repeats the most recently moved key, even while that key is held still. Two keys moving at the same moment interleave.
2. **Lost releases.** A key that loses the stream is never reported again, so its release isn't either.
3. **Steady keys keep the stream.**
   - If the held key is still, the stream stayed on it for 40 of 54 new presses.
   - If the held key is moving, it switched to the new key 95 of 109 times.
   - Re-sending `66` (or `67` then `66`) doesn't change this.
4. **Buffer race.** About 1.5% of packets are sent while the firmware clears its buffer. Either the header is zeroed, or the packet has a `55 FB` header with stroke = 0 and zeroed or half-zeroed travel. Every genuine packet has stroke 34, so Depthpad rejects any stroke outside 10..50.
5. **Typing keeps working** while the stream is on.
6. **Release depth.** A key-up event arrives at a consistent depth, about 1.4–1.5 mm with default settings.

## How Depthpad combines the two sources

Each key's depth is decided like this:

- **Stream packet in the last 30 ms:** use the stream value, which is measured and exact.
- **Otherwise, key down** (per the keyboard's own key events): keep the last streamed depth, or full travel if the press was never streamed.
- **Key-up for a key the stream wasn't following:** start from the learned release depth and ease to 0 over 100 ms.
- **Otherwise:** released.

Key events come from Windows Raw Input and are filtered to this keyboard's device path. While key blocking is on they come from the keyboard hook instead, because a key blocked in a low-level hook never reaches Raw Input.

`tools/check_log.py` scores a recorded session against the key events, and `tools/compare_halljoy.py` runs the same recording through HallJoy's two stream-only rules for comparison.

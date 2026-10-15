# Adding a keyboard

Depthpad separates what differs between keyboards from what doesn't:

| Part | Where | What it does |
|---|---|---|
| **Model file** | `crates/depthpad-core/data/devices/<id>.json` | Name, USB IDs, which driver, key travel, keys and how to draw them |
| **Protocol** | `crates/depthpad-core/src/protocols/` | Pure parsing: requests to send and how to read the keyboard's reports. No I/O, unit-tested |
| **Driver** | `crates/depthpad-core/src/platform/drivers/` | The I/O: opens the keyboard, runs the protocol, reports key depths to the engine |

Everything after the driver (combining key events, sticks, curves, mapping, blocking, the virtual controller) is shared.

## A keyboard whose protocol Depthpad already has

Only a model file is needed. Copy the closest existing one and change:

```jsonc
{
  "id": "brand-model",                 // unique, lower case
  "name": "Brand Model",
  "vendor": "Brand",
  "driver": "keychron",                // an existing driver (see platform/drivers/mod.rs)
  "status": "untested",                // "tested" once a maintainer has tried it, "community" when owners report it works
  "usb": [{ "vid": "3434", "pid": "0B10", "usage_page": "FF60", "usage": "0061" }],
  "travel_mm": 3.5,                    // full key travel
  "width_units": 16,                   // drawing width in key units
  "note": "Shown with the keyboard, e.g. software that must be running.",
  "rows": [ ... ],                     // the drawing, see below
  "keys": { "26": { "hid": 26, "key": "KeyW" }, ... }
}
```

In `usb`, a missing `pid`, `usage_page` or `usage` matches any; `report_id` requires that input report on the collection.

**Keys.** The object keys are the driver's own key numbers ("positions"). For drivers that report HID usages (all the AnalogSense ones), the position is the usage. `hid` is the usage the key sends from the factory (0 for keys handled inside the keyboard, like Fn), and `key` is the [`KeyboardEvent.code`](https://developer.mozilla.org/docs/Web/API/UI_Events/Keyboard_event_code_values) name profiles use, so profiles work across keyboards.

**Rows** follow [keyboard-layout-editor.com](http://www.keyboard-layout-editor.com)'s conventions, so layouts can be copied from there or from [VIA](https://github.com/the-via/keyboards) definitions. Each key is `[position, "legend", width]`, or an object for anything else:

```jsonc
{ "pos": 87, "legend": "+", "w": 1, "h": 2, "x": 0.25, "y": 0.25 }
//  x: gap before the key   y: moves this row and the ones after it down   w/h: size in key units
```

Then add the file to `BUNDLED` in `crates/depthpad-core/src/device.rs` and run the tests. `definitions_are_consistent` checks the file: every drawn key is in `keys` and the other way round, no two keys overlap, nothing is wider than `width_units`, key names are unique.

To check the drawing, run the UI preview (`npm run dev`) and open `http://localhost:1420/preview.html?page=mapping&device=<id>`.

## A new protocol

1. **Learn the protocol.** A [keyboard report](KEYBOARD_REPORTS.md) gives the USB details and anything the keyboard sends on its own; a USB capture of the vendor's software gives the commands that switch analog data on. Only ever send a keyboard commands you've seen its own software send: unknown commands can change settings or start a firmware update.
2. **Write the protocol** in `protocols/`, as pure functions or a type implementing `protocols::analogsense::AnalogProtocol` if it fits that shape (requests out, the set of pressed keys in). Add unit tests with real reports from the capture.
3. **Write or reuse a driver.** Keyboards that report every pressed key can use the shared `analog` driver: add a match arm in `protocols::analogsense::for_driver` and in `platform::drivers::start`. Keyboards that need something different get their own driver implementing `platform::drivers::Session` (see `sonix_ajazz.rs`).
4. **Add the model file** as above.

## Testing without the keyboard

The demo keyboard plays a recorded session back as if it were connected:

```
set DEPTHPAD_DEMO_KEYBOARD=C:\path\to\recording.log
target\release\depthpad.exe
```

Recordings come from **About → Record a session** (they contain every key pressed while recording, so review them before sharing).

## Status

New models start as `untested`. Owners confirming it works move it to `community`; a maintainer trying it on real hardware makes it `tested`.

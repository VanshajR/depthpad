# Third-party notices

Depthpad is licensed under the GNU Affero General Public License v3.0 (see `LICENSE`).
Its source code is available at https://github.com/VanshajR/depthpad.

Depthpad includes or is derived from the following works.

## HallJoy

Parts of Depthpad are ported from HallJoy (https://github.com/PashOK7/HallJoy), AGPL-3.0:

- the stick resolution logic (`crates/depthpad-core/src/sticks.rs`, from `configured_xusb_builder.cpp`);
- the key-blocking rules (`crates/depthpad-core/src/routes.rs`, from `app.cpp` and `block_keys_policy.h`);
- the key-assignment rules (`assigned_usage` in `crates/depthpad-core/src/protocols/sonix_ajazz.rs`, from `aula_mini60_native_model.h`);
- the keyboard session handling: confirmed start and stop, clearing and reconnecting (`crates/depthpad-core/src/platform/drivers/sonix_ajazz.rs`, from `aula_mini60_diagnostic.cpp`);
- the AULA MINI 60 HE (base, PRO, MAX) and AJAZZ AK820 MAX HE key maps, device-info lengths, identity checks and freshness window (`crates/depthpad-core/data/devices/aula-mini60-he*.json`, `ajazz-ak820-max-he.json`, `SonixOptions` in `device.rs`, from `aula_mini60_native_model.h`).

## ViGEmBus (bundled installer)

The installer includes Nefarius's signed ViGEmBus installer, unmodified, and runs it only if you agree.
Copyright (c) Benjamin Höglinger-Stelzer (Nefarius Software Solutions e.U.), BSD-3-Clause.
https://github.com/nefarius/ViGEmBus

## vigem-client

Rust client for ViGEmBus, Copyright (c) Casper (CasualX), MIT License.
https://github.com/CasualX/vigem-client

## Tauri, Svelte and other libraries

The app is built with Tauri (https://tauri.app, MIT or Apache-2.0) and Svelte (https://svelte.dev, MIT), and uses
Rust crates and npm packages listed in `Cargo.lock` and `package-lock.json`, among them hidapi-rs (MIT) with
HIDAPI (BSD-style licence option, https://github.com/libusb/hidapi), windows-rs (MIT or Apache-2.0), serde
(MIT or Apache-2.0) and parking_lot (MIT or Apache-2.0). Their licence texts are in their source packages.

## Lucide icons

Interface icons are from Lucide (https://lucide.dev), ISC License. Copyright (c) Lucide Contributors 2022; portions
copyright (c) Cole Bemis 2013-2022 (Feather, MIT). The full text is in `ui/public/licenses/LICENSE-lucide.txt`.

## AJAZZ key layout data

Key positions for the AK680 V2 (`crates/depthpad-core/data/devices/ajazz-ak680-v2-he.json`) are factual layout data,
taken from the configuration that AJAZZ's web configurator publishes for this model.

## The Python prototype (`prototype/`, not part of the app)

The prototype uses Qt for Python (PySide6) and Qt under the GNU LGPL-3.0 (texts in `licenses/`), cython-hidapi and
HIDAPI (BSD-style option), Python (PSF License), PyInstaller (GPL-2.0-or-later with the bootloader exception) and
ViGEmClient (MIT, https://github.com/nefarius/ViGEmClient; the DLL isn't in the repository, see `prototype/`).

## Trademarks

AJAZZ, Wooting, Razer, NuPhy, DrunkDeer, Keychron, Lemokey, MadLions and Redragon are trademarks of their owners.
Xbox and Windows are trademarks of Microsoft. PlayStation and DualShock are trademarks of Sony Interactive
Entertainment. Depthpad is not affiliated with or endorsed by any of them.

## AnalogSense JavaScript SDK

The analog protocols in `crates/depthpad-core/src/protocols/analogsense.rs` and the tables in
`analogsense_tables.rs` are ported from https://github.com/AnalogSense/JavaScript-SDK.

```
MIT License

Copyright (c) 2024 Calamity, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

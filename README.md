<p align="center"><img src="docs/icon.png" width="96" alt=""></p>

<h1 align="center">Depthpad</h1>

<p align="center">Turn a Hall-effect keyboard into an analog game controller.<br>
How far you press a key becomes how far the stick moves or the trigger pulls.</p>

<p align="center"><img src="docs/screenshot-live.png" width="820" alt="Depthpad's live view: the keyboard with pressed keys lit by depth, and the virtual controller"></p>

## What it does

- **Analog movement from your keys.** Press W lightly to walk, fully to run. Ease into the throttle and brake in racing games.
- **Works in any game that supports a controller.** Depthpad creates a virtual Xbox 360 or PlayStation (DualShock 4) controller.
- **Map any key** to any stick direction, trigger or button by dragging, with a live view of what the game receives.
- **Shape the response** with a draggable curve, presets (Linear, Aggro, Slow, Smooth, Steep, Instant, Turbo) and an anti-deadzone.
- **Profiles** with hotkeys to switch between them, an on-screen badge and sound when you do, and **app linking** that picks a profile when a game is in front.
- **Optional key blocking**, so games see only the controller. Right Ctrl + P pauses everything so you can type.
- **Light on resources.** Closed to the tray it uses about 6 MB of memory and 1% of one CPU core.

<p align="center">
  <img src="docs/screenshot-mapping.png" width="49%" alt="Mapping keys to controller inputs">
  <img src="docs/screenshot-curve.png" width="49%" alt="Response curve editor with presets">
</p>

## Install

1. Download `Depthpad_x.y.z_x64-setup.exe` from [Releases](https://github.com/VanshajR/depthpad/releases).
2. Run it. If the ViGEmBus controller driver is missing, the installer offers to add it (Windows asks for permission).
3. Connect the keyboard **with the USB cable**. Analog data isn't sent over wireless.

> Windows may say "Windows protected your PC", because Depthpad isn't code-signed yet. Choose **More info → Run anyway**. Each release lists SHA-256 checksums so you can check your download.

Requires Windows 10 or 11 (64-bit).

## Keyboards

| Keyboard | Status |
|---|---|
| AJAZZ AK680 V2 HE (`0C45:80BC`) | **Tested** |
| Wooting (any board) | Untested |
| Razer Huntsman V2 Analog, Mini Analog, V3 Pro, V3 Pro TKL, V3 Pro Mini (Razer Synapse must be running) | Untested |
| NuPhy HE boards | Untested |
| DrunkDeer boards | Untested |
| Keychron Q1 HE, Q3 HE, Q5 HE, K2 HE · Lemokey P1 HE | Untested |
| MadLions MAD60HE, MAD68HE, MAD68R | Untested |
| Redragon K709 HE | Untested |

**Untested** means the support is written from [AnalogSense](https://github.com/AnalogSense/JavaScript-SDK)'s working code, but nobody has tried it with Depthpad yet. If you own one, please try it and [tell us how it went](https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml).

**Your keyboard isn't listed?** Open Depthpad → Settings → Keyboard → **Create report** (or run `depthpad-probe.exe` from the release). It records what your keyboard sends during a few guided key presses, without changing anything on it, and saves a file to attach to a [keyboard support request](https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml). See [docs/KEYBOARD_REPORTS.md](docs/KEYBOARD_REPORTS.md).

## Good to know

- **AJAZZ AK680: one key at a time is fully analog.** The keyboard only reports the depth of the key that moved last. Hold W still and press D, and D can't be measured, so it counts as pressed to its actuation point (shown amber). Moving both keys keeps both analog.
- **Key blocking affects every keyboard.** Windows doesn't say which keyboard a key came from.
- **The switch badge needs windowed or borderless games.** Nothing can draw over exclusive fullscreen without hooking the game. The sound cue works everywhere.
- **Games running as administrator** hide their keys from normal apps. If hotkeys or blocking stop working in one, run Depthpad as administrator too.
- **Anti-cheat.** Virtual controllers are widely used (Steam Input and DS4Windows work the same way), but check a competitive game's rules before using one.

## How it works

Depthpad reads each key's travel from the keyboard over USB, combines it with the keyboard's normal key events, shapes it with your profile, and sends the result to a virtual controller through [ViGEmBus](https://github.com/nefarius/ViGEmBus). The AK680 protocol notes are in [docs/PROTOCOL.md](docs/PROTOCOL.md).

Built with Rust (the engine and drivers) and [Tauri](https://tauri.app) with [Svelte](https://svelte.dev) (the window).

## Contributing

Contributions are welcome, especially support for more keyboards. Start with [CONTRIBUTING.md](CONTRIBUTING.md); adding a keyboard is described in [docs/ADDING_A_KEYBOARD.md](docs/ADDING_A_KEYBOARD.md), and building in [docs/BUILDING.md](docs/BUILDING.md).

## Credits

- [HallJoy](https://github.com/PashOK7/HallJoy) by PashOK7: stick behaviour, key-blocking rules and keyboard session handling (AGPL-3.0).
- [AnalogSense](https://github.com/AnalogSense/JavaScript-SDK) by Calamity, Inc.: analog protocols for Wooting, Razer, NuPhy, DrunkDeer, Keychron, Lemokey, MadLions and Redragon boards (MIT).
- [ViGEmBus](https://github.com/nefarius/ViGEmBus) by Nefarius Software Solutions: the virtual controller driver.

## Licence

GNU AGPL-3.0. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Depthpad is not affiliated with AJAZZ, Wooting, Razer, NuPhy, DrunkDeer, Keychron, Lemokey, MadLions, Redragon or Microsoft.

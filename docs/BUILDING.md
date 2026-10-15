# Building Depthpad

## Requirements

- Windows 10 or 11, x64
- [Rust](https://rustup.rs) (stable, MSVC toolchain) with the Visual Studio C++ build tools
- [Node.js](https://nodejs.org) 22 or later
- WebView2 (built into Windows 11 and current Windows 10)
- The [ViGEmBus](https://github.com/nefarius/ViGEmBus/releases) driver, to use the virtual controller

## Run it

```
npm ci
npx tauri dev
```

## Tests and checks

```
cargo test --workspace
npm run check
```

Hardware checks (with the keyboard connected, Depthpad closed):

```
cargo run --release -p depthpad-core --example selftest     # connect, stream, controller
cargo run --release -p depthpad-core --example padcheck     # plug in each virtual controller kind
```

## The installer

The installer bundles Nefarius's ViGEmBus installer, which isn't kept in the repository. Download it and check it:

```
curl -L -o src-tauri/redist/ViGEmBus_1.22.0_x64_x86_arm64.exe https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe
certutil -hashfile src-tauri\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe SHA256
```

The SHA-256 must be `89220a7865076b342892f98865f3499fb7c4cfd673159e89d352c360fd014c6a`. Then:

```
npx tauri build
```

The installer is written to `target/release/bundle/nsis/`. The [release workflow](../.github/workflows/release.yml) does all of this when a `v*` tag is pushed and creates a draft release with the installer, a portable zip, the keyboard report tool and checksums.

## UI preview

`npm run dev` and open `http://localhost:1420/preview.html` to work on the interface without the app or a keyboard. Query options: `page` (live, mapping, settings, about), `view` (0 controller, 1 analysis), `key` (selected key position), `device` (a model id), `theme`, `accent`, `kbs=2&choose=1` (two keyboards, with the chooser).

## Layout

| Path | |
|---|---|
| `crates/depthpad-core` | Engine, protocols, drivers, profiles (Rust) |
| `crates/depthpad-probe` | The keyboard report tool |
| `src-tauri` | The app: window, tray, settings, commands |
| `ui` | The interface (Svelte) |
| `prototype` | The original Python prototype, kept for reference and its analysis tools |

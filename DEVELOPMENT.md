# Development

Notes for building and hacking on Sennheiser Control. For how the headset protocol works, see
[`PROTOCOL.md`](PROTOCOL.md).

## Build and test

Requires Rust (MSVC toolchain, pinned in `rust-toolchain.toml`) and the Visual Studio Build Tools.

```
cargo build --release -p app        # target\release\sennheiser-control.exe (≈9 MB, single file)
cargo test --workspace              # protocol, engine (incl. simulator end-to-end), app
cargo run -p hsctl -- info          # CLI: dump everything the headset reports
```

## Layout

```
crates/protocol   GAIA framing, typed requests/updates, EQ math, products and capabilities (pure Rust, unit-tested)
crates/headset    shared core: headset state, refresh reads, action -> GAIA writes, simulated MOMENTUM 4 (pure Rust)
crates/bluetooth  WinRT device watcher + RFCOMM link
crates/engine     GAIA client, session worker, engine actor (Windows side of the core)
crates/web        wasm-bindgen wrapper of the core for the web app
app               eframe/egui window, tray thread (own message loop), settings, Windows glue
tools/hsctl       bring-up CLI: scan / info / get / set / monitor / eq / engine / raw
re/               reverse-engineering artefacts: device model JSON and helper scripts (decompiled app code stays local)
assets/           app.ico (generated) and the logo SVG it is drawn from
web/              Next.js web app (Web Serial); see web/README.md
```

Protocol and behaviour changes go into `crates/protocol` / `crates/headset` once and reach both apps. After changing
them, run `pnpm core` in `web/` to rebuild the committed WebAssembly in `web/lib/core/`.

## Command-line flags

| Flag | Effect |
|---|---|
| `--minimized` | start in the tray, like the "Start minimized" setting (which also applies to Start with Windows) |
| `--quit` | ask the running instance to exit |
| `--sim` | use a simulated headset (UI work without hardware). Runs as its own instance with `settings-sim.json` |
| `--debug` | verbose log, including every GAIA frame (`%APPDATA%\SennheiserControl\app.log`) |
| `--render-icons DIR` | write tray-icon preview sheets and `app.ico`. `--render-icons assets` refreshes the icon |

## Dev-only environment variables

- `HC_SIM_BATTERY=12`: initial battery level for `--sim`
- `HC_SIM_DEVICES=2`: number of simulated headsets
- `HC_SIM_NOISE=custom|off`: start the simulated headset in that noise mode
- `HC_SIM_EQ=custom`: start the simulated headset with the user EQ on and a curve that matches no preset
- `HC_DEV_HEIGHT=1600`: initial window height
- `HC_DEV_PAGE=settings|devices|panels`: open the settings page, the expanded headset switcher, or every
  collapsible panel and confirmation
- `HC_DEV_SCROLL=1500`: initial scroll offset of the main page
- `HC_PRINT_MENU=1 cargo test -p app menu -- --nocapture`: print the whole tray menu tree
- `SENTRY_DSN=https://…`: Sentry project DSN. Read at runtime if set, otherwise baked in at
  compile time if set during `cargo build`. Unset = crash reporting runs disabled (local logs only).

## Crash reporting (Sentry)

The desktop app reports panics and `log::error!` lines to Sentry. Warnings become breadcrumbs.
No tracing, no PII (`send_default_pii(false)`), opt-out in Settings → Privacy
(`crash_reporting: true` by default in `%APPDATA%\SennheiserControl\settings.json`).

- Code: `app/src/crash_report.rs` (`init` must run before `init_logging` so the panic hook
  chains onto Sentry's handler; the guard is kept for the process lifetime).
- Release builds set `[profile.release] debug = true` so Windows PDBs exist for symbolication.
  Upload them with `sentry-cli debug-files upload --include-sources .` before/while shipping.
- Verify: run with a real DSN and trigger a test error (e.g. `SENTRY_DSN=… cargo run -p app -- --sim`
  plus a forced `panic!`), then check the issue title/message/URL in Sentry.

## Where things are stored

- Settings: `%APPDATA%\SennheiserControl\settings.json`; `--sim` uses `settings-sim.json` next to it.
- Logs, same folder: `app.log` (everything; `--debug` adds GAIA frames) and `sennheiser-control-error.log` (warnings,
  errors and crashes with a backtrace, created on the first one). With an untested model, the error log also gets the
  model id and firmware at connect and every command the headset rejected. Each log moves to `*.old.log` once it
  gets big.
- Start with Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SennheiserControl`.
- Builds from before the rename used `HeadsetControl` for both. The first start moves the old settings folder and
  autostart entry over.

## Safety

The engine refuses firmware update/DFU, pairing-list wipes, MMI rewrites, debug and transport re-negotiation
commands (`protocol::is_dangerous_packet`). Factory reset and deleting a single pairing are on that list too; the
only way to send them is a confirmed button in the window. Only `hsctl raw … --unsafe` can bypass the list.

Write commands are only added once their format is confirmed from the official apps (device model JSON or
decompiled code); unverified ones (rename, multipoint switch, prompt volume/language) stay out.

## Known gaps

- Only the MOMENTUM 4 has been tested on hardware. The other products in `crates/protocol/src/product.rs` are built
  from Smart Control's device models and product code (PROTOCOL.md §8). Their feature sets live in
  `Capabilities::new`.

- On firmware older than 3.18 the EQ uses the "gen 1" encoding. It matches the official app byte for byte, but is
  untested on hardware because the test unit runs 3.38.3.

## Cutting a release

Build from a clean tree. Use a separate target dir if a copy of the app is running, since it locks the exe:

```
cargo build --release -p app --target-dir target/dist
sha256sum target/dist/release/sennheiser-control.exe
gh release create vX.Y.Z --target main --title "Sennheiser Control X.Y.Z" sennheiser-control.exe sennheiser-control.exe.sha256
```

Bump the version in `Cargo.toml` (workspace) and `app/app.rc` first.

## Linux build

Prereq: a C `libdbus` for linking (`sudo dnf install dbus-devel openssl-devel` /
`sudo apt install libdbus-1-dev libssl-dev`). The tray uses tray-icon's `ksni`
backend, pure Rust D-Bus, so no GTK/AppIndicator `-devel` packages are needed.
The window builds eframe with `wgpu` (Vulkan) instead of `glow`: glow finds no GL config on
NVIDIA Wayland; eframe/winit otherwise dlopen their Wayland/X11 libs at runtime.

```
cargo build --release -p app   # target/release/sennheiser-control (~20 MB, single file)
cargo run -p app               # pair + connect the headset in system Bluetooth settings first
cargo run -p app -- --sim      # simulated headset, no hardware needed
cargo run -p hsctl -- scan     # list paired headsets (BlueZ check)
cargo test --workspace
cargo check --target x86_64-pc-windows-msvc --workspace   # Windows still intact
```

Bluetooth is BlueZ via `bluer`: discovery polls paired devices (2 s diff loop, same
`DiscoveryEvent` shape as WinRT), RFCOMM resolves the GAIA channel with a hand-rolled SDP
query over L2CAP and connects directly. The headset exposes the GAIA UUID on two channels —
only one runs GAIA (it answers, the other drops instantly), so connect probes candidates in
order and keeps the first link that survives 700 ms. Quirks found: bluer L2CAP needs
`SeqPacket::connect` (binds first; raw `Socket::connect` yields ENOTCONN on send), and the
fresh channel needs ~500 ms before the first send lands.

Install (per user): copy the binary to `~/.local/bin`, `assets/linux/*.png` to
`~/.local/share/icons/hicolor/{16,24,32,48,64,128,256}x{...}/apps/` (renamed to
`sennheiser-control.png`) and `assets/linux/sennheiser-control.desktop` to
`~/.local/share/applications/`. Autostart is handled in-app (writes `~/.config/autostart/`).

Settings and logs live in `~/.config/SennheiserControl/` (`settings.json`, `app.log`,
`sennheiser-control-error.log`); the theme follows GNOME's color-scheme when readable.

Known gaps: GNOME needs an AppIndicator/SNI shell extension for the tray icon (KDE works out
of the box). Second-launch window showing and `--quit` are not wired up on Linux yet (the
lockfile only blocks a second instance; quit via the tray menu).

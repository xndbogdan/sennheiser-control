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
crates/protocol   GAIA framing, typed requests/updates, EQ math, capabilities (pure Rust, unit-tested)
crates/bluetooth  WinRT device watcher + RFCOMM link
crates/engine     GAIA client, session worker, engine actor, simulated MOMENTUM 4
app               eframe/egui window, tray thread (own message loop), settings, Windows glue
tools/hsctl       bring-up CLI: scan / info / get / set / monitor / eq / engine / raw
re/               reverse-engineering artefacts: device model JSON and helper scripts (decompiled app code stays local)
assets/           app.ico (generated) and the logo SVG it is drawn from
```

## Command-line flags

| Flag | Effect |
|---|---|
| `--minimized` | start in the tray (used by "Start with Windows") |
| `--quit` | ask the running instance to exit |
| `--sim` | use a simulated headset (UI work without hardware). Runs as its own instance with `settings-sim.json` |
| `--debug` | verbose log, including every GAIA frame (`%APPDATA%\SennheiserControl\app.log`) |
| `--render-icons DIR` | write tray-icon preview sheets and `app.ico`. `--render-icons assets` refreshes the icon |

## Dev-only environment variables

- `HC_SIM_BATTERY=12`: initial battery level for `--sim`
- `HC_SIM_DEVICES=2`: number of simulated headsets
- `HC_SIM_NOISE=custom|off`: start the simulated headset in that noise mode
- `HC_DEV_HEIGHT=1600`: initial window height
- `HC_DEV_PAGE=settings|devices|panels`: open the settings page, the expanded headset switcher, or every
  collapsible panel and confirmation
- `HC_DEV_SCROLL=1500`: initial scroll offset of the main page
- `HC_PRINT_MENU=1 cargo test -p app menu -- --nocapture`: print the whole tray menu tree

## Where things are stored

- Settings: `%APPDATA%\SennheiserControl\settings.json`; `--sim` uses `settings-sim.json` next to it.
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

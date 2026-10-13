# Sennheiser Control

A small Windows tray app for the **Sennheiser MOMENTUM 4**:
- battery level in the notification area, refreshed every minute (as Smart Control Plus does) and on every change
  the headset reports
- noise control (Adaptive / Custom / Off) from a right-click menu
- a settings window with the same features as Sennheiser's Smart Control Plus app for the M4: noise control with
  the Custom noise-cancelling ⟷ transparency balance, wind reduction, 5-band EQ with presets and bass boost,
  podcast mode, sound personalization, Hi-Res 24 bit/96 kHz, call settings, on-head detection, touch lock and the
  gesture reference, auto power-off, tones and voice prompts, multipoint connections (connect, disconnect,
  forget), and factory reset

It talks to the headset directly over Bluetooth (GAIA v3 over RFCOMM). There is no phone and no cloud involved.
Everything here was reverse-engineered from the official Smart Control app; see [`PROTOCOL.md`](PROTOCOL.md).

> Unofficial. Not affiliated with or endorsed by Sennheiser or Sonova.
> The name and the logo in `assets/sennheiser-logo.svg` (app icon) belong to Sennheiser and are used here for
> personal use only.

## Use

1. Pair the MOMENTUM 4 with Windows (Settings → Bluetooth & devices) and connect it.
2. Run `sennheiser-control.exe`. It finds the headset automatically and shows the battery in the tray.
3. Interact with the tray icon:
   - **Left-click:** show or hide the window.
   - **Right-click:** Adaptive / Custom / Off, the Custom level, wind reduction, EQ preset, bass boost,
     podcast mode, headset settings, multipoint connections, the headset switcher (when several are connected),
     Start with Windows, Quit.
4. Closing or minimizing the window keeps the app in the tray. Use Quit in the tray menu to exit, or change the
   close button behaviour in Settings.

| Flag | Effect |
|---|---|
| `--minimized` | start in the tray (used by "Start with Windows") |
| `--quit` | ask the running instance to exit |
| `--sim` | use a simulated headset (UI work without hardware). Runs as its own instance with `settings-sim.json` |
| `--debug` | verbose log (`%APPDATA%\SennheiserControl\app.log`) |
| `--render-icons DIR` | dev: write tray-icon preview sheets and `app.ico` (`--render-icons assets` refreshes the icon) |

Settings live in `%APPDATA%\SennheiserControl\settings.json`. Start-with-Windows uses
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SennheiserControl`. Builds from before the rename used
`HeadsetControl` for both. The first start moves the old settings folder and autostart entry over.

Dev-only environment variables:
- `HC_SIM_BATTERY=12`: initial battery level for `--sim`
- `HC_SIM_DEVICES=2`: number of simulated headsets
- `HC_DEV_HEIGHT=1600`: initial window height
- `HC_DEV_PAGE=settings|devices|panels`: open the settings page, the expanded headset switcher, or every
  collapsible panel and confirmation
- `HC_DEV_SCROLL=1500`: initial scroll offset of the main page
- `HC_SIM_NOISE=custom|off`: start the simulated headset in that noise mode

### Notes

- **Low-battery alert** uses a standard Windows notification. It only appears if notifications are enabled
  (Settings → System → Notifications). The tray number also turns red at 15% or below.
- **Noise modes** follow Smart Control Plus: *Adaptive* lets the headset manage noise cancelling, *Custom* sets
  a fixed balance from full noise cancelling to full transparency, and *Off* (fw 2.13.32+) turns both off.
- **Factory reset** (About this headset) clears every setting and pairing on the headset, including this PC. It
  asks for confirmation first.
- On firmware older than 3.18 the EQ uses the "gen 1" encoding. It is byte-for-byte identical to the official
  app, but untested on hardware, because the test unit runs 3.38.3.

## Build

Requires Rust (MSVC toolchain, pinned in `rust-toolchain.toml`) and the Visual Studio Build Tools.

```
cargo build --release -p app        # target\release\sennheiser-control.exe (≈9 MB, single file)
cargo test --workspace              # protocol, engine (incl. simulator end-to-end)
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
```

Safety: the engine refuses firmware update/DFU, pairing-list wipes, MMI rewrites, debug and transport
re-negotiation commands (`protocol::is_dangerous_packet`). Factory reset and deleting a single pairing are on that
list too. The only way to send them is a confirmed button in the window. Only `hsctl raw … --unsafe` can bypass the list.

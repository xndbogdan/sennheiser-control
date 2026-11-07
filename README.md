# Sennheiser Control

A small Windows tray app I slopped into existence in my free time to get a semblance of control of the
**Sennheiser MOMENTUM 4** headphones under Windows, because Sennheiser didn't think anyone would use these with a
computer or something.

You only need to connect the headphones to your computer and start this up.

![The main window](screenshots/main-window.jpeg)

## What it does

- Battery level in the tray
- Noise control: Adaptive, Custom (noise cancelling ⟷ transparency) and Off, plus wind reduction
- EQ: Neutral, Sennheiser's presets, your own presets and a Custom curve, plus bass boost and podcast mode
- Pretty much everything else the Smart Control Plus app does for the M4: Hi-Res, call settings, on-head
  detection, touch lock, auto power-off, prompts, multipoint, factory reset

Enough of it works to keep me satisfied, and I didn't care to investigate anything beyond that. If you find
something that doesn't work, [open an issue](https://github.com/xndbogdan/sennheiser-control/issues) and complain
there.

## In the browser

There's also a web version at **[sennheiser-control.vercel.app](https://sennheiser-control.vercel.app/)**: same
controls, no install, over Bluetooth from Chrome or Edge on desktop (Windows, macOS, Linux). Its code is in
[`web/`](web/README.md). It runs the exact same protocol code as the tray app, compiled to WebAssembly, and
has a demo mode with a simulated headset. What it can't do is live in the tray: it only works while the tab is open.

## Other Sennheiser headphones (experimental)

These speak the same protocol as the M4, so the app tries them too. Each one only gets the features Sennheiser's own
app offers for it:

- ACCENTUM Wireless and ACCENTUM Plus Wireless
- ACCENTUM True Wireless
- MOMENTUM True Wireless 3 and 4
- MOMENTUM Sport

I only own an M4, so none of these have been tested. Everything is taken from how Sennheiser's app talks to them.
If something misbehaves, attach `%APPDATA%\SennheiserControl\sennheiser-control-error.log` to your issue. With
these models the app writes the model and firmware there when it connects, followed by every command the headset
turns down and any crash.

Not supported: the CX earbuds (Sennheiser's app reaches those over Bluetooth LE only), and older models like the
MOMENTUM 3, PXC 550, HD 450BT or the first two MOMENTUM True Wireless, which use a different protocol.

## Getting started

1. Pair the headphones with Windows and connect them.
2. Grab `sennheiser-control.exe` from [Releases](https://github.com/xndbogdan/sennheiser-control/releases) and
   run it. There's no installer. Windows will grumble because it isn't signed: *More info → Run anyway*.
3. Left-click the tray icon for the window, right-click for the quick menu. Closing the window keeps the app in
   the tray; *Quit* is in the menu. Want it at login? Tray menu → *Start with Windows*. Tick *Start minimized* too
   if it should start in the tray without opening the window.

   ![The tray menu](screenshots/tray-menu.png)

### Linux

There's no release binary yet — build it yourself (needs Rust 1.95+, `dbus-devel` /
`libdbus-1-dev` for the BlueZ D-Bus link and `openssl-devel` / `libssl-dev`
for crash-report uploads): `cargo build --release -p app`, then copy
`target/release/sennheiser-control` to `~/.local/bin`, `assets/linux/*.png` to
`~/.local/share/icons/hicolor/` and `assets/linux/sennheiser-control.desktop` to
`~/.local/share/applications/`. Pair and connect the headphones in your desktop's Bluetooth
settings first. Details and quirks are in [`DEVELOPMENT.md`](DEVELOPMENT.md#linux-build).
Tested on Fedora/KDE with M4 firmware 3.37.3.

## Good to know

- Low-battery alerts are regular Windows notifications, so they only show up if notifications are on.
- Tested on M4 firmware 3.38.3.
- Crashes and errors go to `%APPDATA%\SennheiserControl\sennheiser-control-error.log`. Anonymous crash and
  error reports are also sent to Sentry by default (Settings → Privacy to opt out, no personal data).
- Not there yet: renaming the headphones, the multipoint on/off switch, prompt volume.

## Nerd stuff

Building, flags and code layout are in [`DEVELOPMENT.md`](DEVELOPMENT.md). How the headset protocol works (all
reverse-engineered from Sennheiser's apps) is in [`PROTOCOL.md`](PROTOCOL.md).

## Contributing

Found a bug, want a feature, or have anything else in mind? Just open a pull request and I'll look it over.

A few things that make that easier:

- Headset behaviour lives in the Rust crates (`crates/protocol`, `crates/headset`) and is shared by the tray app and
  the web version, so a fix there lands in both. If you change it, run `pnpm core` in `web/` and commit the rebuilt
  `web/lib/core/` too.
- Run `cargo test --workspace` (and `pnpm lint && pnpm build` in `web/` if you touched it) before opening the PR.
- New write commands should come from Sennheiser's own apps (see [`PROTOCOL.md`](PROTOCOL.md)), not from guessing:
  a wrong write goes to someone's real headphones.

---

Unofficial, not affiliated with Sennheiser. For my personal use and pleasure, uses the Sennheiser logo because it
looks nice. Will remove if they complain I guess.

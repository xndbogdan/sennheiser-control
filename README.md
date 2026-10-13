# Sennheiser Control

A small Windows tray app I slopped into existence in my free time to get a semblance of control of the
**Sennheiser MOMENTUM 4** headphones under Windows, because Sennheiser didn't think anyone would use these with a
computer or something.

It talks to the headphones directly over Bluetooth. No phone, no account, no cloud.

## What it does

- Battery level in the tray
- Noise control: Adaptive, Custom (noise cancelling ⟷ transparency) and Off, plus wind reduction
- EQ: Neutral, Sennheiser's presets, your own presets and a Custom curve, plus bass boost and podcast mode
- Pretty much everything else the Smart Control Plus app does for the M4: Hi-Res, call settings, on-head
  detection, touch lock, auto power-off, prompts, multipoint, factory reset

## Getting started

1. Pair the MOMENTUM 4 with Windows and connect it.
2. Grab `sennheiser-control.exe` from [Releases](https://github.com/xndbogdan/sennheiser-control/releases) and
   run it. There's no installer. Windows will grumble because it isn't signed: *More info → Run anyway*.
3. Left-click the tray icon for the window, right-click for the quick menu. Closing the window keeps the app in
   the tray; *Quit* is in the menu. Want it at login? Tray menu → *Start with Windows*.

## Good to know

- Low-battery alerts are regular Windows notifications, so they only show up if notifications are on.
- Tested on firmware 3.38.3.
- Not there yet: renaming the headphones, the multipoint on/off switch, prompt volume.

## Nerd stuff

Building, flags and code layout are in [`DEVELOPMENT.md`](DEVELOPMENT.md). How the headset protocol works (all
reverse-engineered from Sennheiser's apps) is in [`PROTOCOL.md`](PROTOCOL.md).

---

Unofficial, not affiliated with Sennheiser. For my personal use and pleasure, uses the Sennheiser logo because it
looks nice. Will remove if they complain I guess.

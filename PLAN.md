# Plan — Windows tray app for Sennheiser MOMENTUM 4 (Rust + egui)

> Historical development plan, kept for its decision records. Current behaviour is described in `README.md`
> and `PROTOCOL.md`; the early spikes it mentions have since been removed.

Goal: a small, good-looking Windows app that finds a connected MOMENTUM 4, shows its battery in the system tray,
and exposes every setting from `PROTOCOL.md`. It lives in the tray when closed or minimized, and the tray
right-click menu covers all settings, the noise mode switch, and Quit.

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `(verify)` = needs real hardware

---

## 1. Decision record: Rust + egui?

**Can we?** Yes. Every piece has a solid crate:

| Need | Crate | Notes |
|---|---|---|
| Bluetooth discovery + RFCOMM | `windows` (windows-rs, WinRT projections) | `Windows.Devices.Enumeration`, `Devices.Bluetooth`, `Devices.Bluetooth.Rfcomm`, `Networking.Sockets`. Fallback: Winsock `AF_BTH` via the same crate |
| Window + UI | `eframe` / `egui` | immediate mode, fully custom-drawn, so we can style it how we want |
| Tray icon + right-click menu | `tray-icon` + `muda` | dynamic RGBA icons, tooltips, submenus, check/radio items |
| Icons in UI | `egui-phosphor` | headphones, battery, wind, ear, sliders, lightning… |
| Tray icon rendering | `tiny-skia` + `ab_glyph` | draw "87" + level bar at the exact tray pixel size |
| Settings | `serde` + `serde_json` | `%APPDATA%\<app>\settings.json` |
| Exe icon / version info | `embed-resource` or `winresource` | MSVC toolchain |
| Win11 polish (optional) | `window-vibrancy`, DWM calls via `windows` | Mica backdrop, rounded corners |

**Should we?** Yes. It fits an always-running tray utility well:

- A single ~5–10 MB native exe with no runtime. Idle CPU is near zero because egui only repaints on events,
  and memory stays small.
- `windows-rs` gives first-class access to the exact WinRT Bluetooth APIs we need.
- The protocol layer is pure Rust, so it unit-tests well with golden frames from `PROTOCOL.md`.

Honest costs, accepted:

- **egui doesn't look like Fluent out of the box.** "Beautiful" means building a custom theme and widgets
  (segmented control, toggles, EQ curve). That work is budgeted in Phase 4.
- **eframe isn't built for tray-first apps.** When the window is hidden, `update()` stops running. Design answer:
  the tray lives on its **own thread with its own message loop**, and shows the window through the raw HWND
  (Win32 `ShowWindow`). Phase 0 checks this first.
- Rejected alternatives: C# WinUI 3 would look more native but has a heavier runtime. Tauri has a webview
  footprint. Neither is a better fit for this app.

## 2. Scope

**v1 (this plan):**

- Detect paired and connected **MOMENTUM 4** headsets. One active control session at a time, with a switcher if
  several are connected.
- Tray: dynamic battery icon, tooltip, and a full right-click menu (all settings, noise mode, open window, quit).
- Window: device header, noise control, EQ (presets, 5-band, bass boost, podcast), settings, device info, and
  app settings.
- Close or minimize sends the window to the tray. Single instance. Optional start with Windows.

**Architected for, not in v1:**

- Other GAIA v3 Sennheiser models: Accentum, Accentum Plus, Accentum TW, MTW3, MTW4, Momentum Sport, CX 200 /
  CX 200 Sports / CX Plus TW / CX 500.
  - All are the same protocol family as the M4; only device profiles and the earbud battery UI (L/R/case) differ.
  - None can be tested without the hardware.
  - Legacy GAIA v1/v2 (HD 250–458, CX 350/400, TW1/TW2, IE 80) and Nautilus (Momentum 3, PXC 550) use
    different stacks and are out of scope.

**Non-goals:** firmware update, factory reset, hearing test (sound-personalization calibration), voice assistant,
statistics or panic logs.

## 3. Architecture

```
┌──────────────────────── main thread (winit / eframe) ────────────────────────┐
│  UI (egui): reads AppState snapshot, sends Action → engine                   │
└───────────────▲──────────────────────────────────────────────┬──────────────┘
                │ state updates (watch channel + repaint)       │ Action
┌───────────────┴───────────── engine thread (device actor) ─────▼─────────────┐
│  Discovery (DeviceWatcher)  →  Session (one per active headset)              │
│  - request/response correlation, priority send queue, timeouts               │
│  - notification dispatch → HeadsetState                                      │
│  - profile (M4): capabilities from FW gates, EQ Gen1/Gen2                    │
└───────────────▲──────────────────────────────────────────────┬──────────────┘
                │ bytes                                        │ state updates
┌───────────────┴───── RFCOMM transport (reader thread) ────┐  ┌▼─────── tray thread ───────────┐
│ StreamSocket / Winsock AF_BTH, GAIA deframer               │  │ own Win32 message loop         │
└────────────────────────────────────────────────────────────┘  │ TrayIcon + muda Menu (owned)   │
                                                                │ renders battery icon, menu →   │
                                                                │ Action / ShowWindow(hwnd)      │
                                                                └────────────────────────────────┘
```

- Channels use `crossbeam-channel`. The engine publishes immutable `AppState` snapshots. The UI thread gets them
  via `ctx.request_repaint()`, and the tray thread gets them via a channel plus a `PostThreadMessageW(WM_APP)`
  wake-up.
- No async runtime: WinRT async operations are blocking-awaited on worker threads.
- **Why the tray gets its own thread:** `TrayIcon` and `muda` menus are `!Send` and must be updated on the thread
  that pumps their messages. Running them independently of eframe means the tray keeps working while the window is
  hidden.

### Workspace layout

```
Cargo.toml                 # workspace
rust-toolchain.toml        # pin MSVC stable (machine default is GNU; MSVC toolchain + VS Build Tools 2022 present)
crates/protocol/           # pure: framing, deframer, ids, typed commands/responses, profiles, EQ math (no Windows deps)
crates/bluetooth/          # Windows: discovery (DeviceWatcher), RFCOMM transport, SDP/service checks
crates/engine/             # session actor, state model, reconnect, action handling, safety deny-list
app/                       # eframe UI, theme/widgets, tray thread, settings, single-instance, autostart (bin)
tools/hsctl/               # CLI for hardware bring-up: scan, get/set, raw, monitor (bin)
assets/                    # app icon (.ico, own design — no Sennheiser logos), Inter font (OFL)
```

### Core types (sketch)

```rust
enum NoiseMode { NoiseCancelling, Transparency, Off /* only if capability probed */ , Mixed(u8) }
enum AntiWind { Off = 0, Max = 1, Auto = 2 }
enum EqGen { Gen1, Gen2 }   // chosen from Service_SystemReleaseVersion: >=3.18.0 → Gen2, >=2.12.0 → Gen1

struct HeadsetState {
    identity: Identity,          // bt name, model id ("M4AEBT Black"), variant, serial, fw (major.minor.patch)
    link: LinkState,             // Disconnected | Connecting | Connected | Error(..)
    battery: Option<Battery>,    // percent, charging state
    codec: Option<Codec>,
    noise: NoiseState,           // adaptive, anti_wind, transparency 0..=100, pause_on_full, anc_status?
    eq: EqState,                 // gen, bands[5] (dB), bass_boost, audio_mode (UserEq|Podcast|Flat), preset
    settings: DeviceSettings,    // on-head, smart pause, auto-answer, comfort call, auto-off, prompts, sidetone, touch lock, bt compat
    caps: Capabilities,          // derived from FW gates in PROTOCOL.md §5 + runtime probes
}
enum Action { SetNoiseMode(NoiseMode), SetTransparency(u8), SetAdaptive(bool), SetAntiWind(AntiWind),
              SetEq { bands: [f32; 5], bass_boost: bool }, ApplyPreset(PresetId), SetAudioMode(AudioMode),
              SetSetting(SettingChange), SelectDevice(DeviceId), Refresh, /* … */ }
```

### Noise-mode semantics (tray "Transparency / NC / Off")

| Mode | Commands | Shown when |
|---|---|---|
| Noise cancelling | `ANC_Transparency = 0` | always |
| Transparency | `ANC_Transparency = <user level, default 100>` | always |
| Off | `ANC_Status (0x1A04) = 0` | only if the Phase 1 probe proves it works on the M4 (verify) |
| *Mixed (n%)* | display only, when 0 < level < chosen transparency level | — |

### Safety rules (enforced in `engine`, not just the UI)

- Deny-list: factory reset `0x0040`, the whole upgrade feature (`0x02xx`), Qualcomm DFU (`0x001D:0x0Cxx`),
  delete pairing list `0x1406`, debug and data-transfer commands. Raw frames are only allowed through
  `hsctl raw --unsafe`.
- EQ writes are disabled until read-back decoding has been confirmed on the device (Phase 1, "EQ safe mode").
- Slider changes are debounced: send at most every ~100 ms while dragging, plus a final send on release.

## 4. Multiple headsets (decision)

- The engine tracks all paired supported headsets and their connection state, but opens **one GAIA session**: the
  "active" headset.
- Auto-select order: the last-used device if it's connected, otherwise the first connected one.
- UI: the header shows the active headset's name. **Only when more than one supported headset is connected** does
  the name become a dropdown (chevron) for switching. Otherwise there's no list clutter.
- Tray: the battery icon and tooltip follow the active headset. A "Headset ▸" submenu with radio items appears only
  when more than one is connected.
- Disconnected state: the window shows an empty state with a list of *paired but not connected* supported headsets,
  plus an "Open Bluetooth settings" button (`ms-settings:bluetooth`).

## 5. Look & feel spec ("make it beautiful")

**Window:** 400×720 logical px (min 360×560), borderless with a custom title bar (drag area, minimize-to-tray,
close-to-tray), Win11 rounded corners via `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE)`, optional Mica.
Single-column card layout, like a phone companion app.

**Theme:** follows the Windows app theme (dark/light), with a manual override.

| Token | Dark | Light |
|---|---|---|
| bg | `#0F1115` | `#F4F5F8` |
| card | `#171A21` | `#FFFFFF` |
| card border | `#232733` | `#E3E5EB` |
| text / secondary | `#E8EAF0` / `#8A90A2` | `#14161C` / `#5D6375` |
| accent | `#5B8CFF` (or Windows accent color, opt-in) | same |
| good / warn / bad | `#3CCB7F` / `#F5A524` / `#F04F5A` | same |

Typography is Inter (embedded), sizes 12/14/16/20/28, with tabular digits for battery and dB values. Cards use a
14 px radius, 16 px padding and 12 px gaps. Motion is 150–200 ms ease-out on toggles, the segmented-control pill
and the battery ring, with no idle animation.

**Custom widgets:** `SegmentedControl` (sliding pill), `Toggle` (animated switch), `AccentSlider` (thick track
with a value bubble), `BatteryRing`, `EqEditor` (5 vertical sliders on top of a smooth curve with a gradient
fill, dB grid, double-click to reset a band), `PresetChips`, `SettingRow` (icon + title + subtitle + trailing
control), `StatusDot`, `Chip`.

**Screen, top to bottom:**

1. **Header card**
   - headphones illustration (vector, drawn by us)
   - name, plus a switcher when several are connected
   - variant ("Black")
   - status dot
   - `BatteryRing` with % and a charging bolt
   - chips: codec (AAC / aptX Adaptive…) and firmware
2. **Noise control**
   - `SegmentedControl` [Noise cancelling · Transparency · Off*]
   - transparency slider 0–100
   - Adaptive ANC toggle
   - Wind reduction segmented [Off · Auto · Max]
   - "Pause music at full transparency" toggle
   - Phone-call sidetone stepper
3. **Sound**
   - Audio mode [EQ · Podcast]
   - `PresetChips` (Flat, Rock, Pop, Dance, Hip-Hop, Classical, Movie, Jazz, plus user presets; saving a custom
     preset is client-side)
   - `EqEditor` (63/250/1k/4k/8k, ±6 dB)
   - Bass boost toggle
   - Reset
4. **Controls & behaviour**
   - On-head detection
   - Smart pause and Auto-answer calls (both disabled with a hint when on-head detection is off)
   - Comfort call
   - Touch controls lock
   - Auto power-off [Never / 15 / 30 / 60 min]
   - Voice prompts [Off / Tones / Voice + tones]
   - Bluetooth mode [Best audio / Best compatibility]
5. **Device info** (collapsible): model ID, serial, firmware, hardware revision, codec, multipoint hosts (read-only in v1).
6. **App** (gear in the title bar opens a sheet):
   - start with Windows
   - start minimized
   - close button behaviour
   - transparency level used by the tray "Transparency" mode
   - low-battery notification threshold
   - theme

Any feature whose capability flag is false (FW gate or failed probe) is hidden, not greyed out. The exception is
settings that depend on another setting, which get an explanatory hint.

### Tray icon spec

- Rendered at runtime at the exact small-icon size for the current DPI (`GetSystemMetricsForDpi(SM_CXSMICON)`:
  16/20/24/32 px). It re-renders on DPI or theme change (`WM_SETTINGCHANGE`, `WM_DPICHANGED`).
- **Connected:** the battery number ("87", bold, tabular) plus a 2 px level bar along the bottom, coloured by level
  (good > 40, warn 16–40, bad ≤ 15).
  - **100%:** a full-battery glyph instead of "100".
  - **Charging:** a bolt replaces the bar.
  - **Unknown:** "–".
- **Disconnected:** a monochrome headphones glyph that follows the taskbar's light or dark setting
  (`SystemUsesLightTheme`).
- **Connecting:** the headphones glyph with a small dot.
- **Tooltip:** `MOMENTUM 4 — 87% · Charging · Noise cancelling` / `No headset connected`.
- Left-click shows or hides the window. Right-click opens the menu.
- Optional toast when the battery drops to or below the threshold.

### Tray right-click menu

```
MOMENTUM 4 — 87%                      (disabled header; "No headset connected" when none)
──────────────
◉ Noise cancelling
○ Transparency
○ Off                                 (only if supported)
Transparency level      ▸  25 / 50 / 75 / 100 %
Adaptive ANC            ✓
Wind reduction          ▸  ◉ Off ○ Auto ○ Max
──────────────
EQ preset               ▸  ◉ Custom ○ Flat ○ Rock ○ Pop ○ Dance ○ Hip-Hop ○ Classical ○ Movie ○ Jazz (+ user presets)
Bass boost              ✓
Podcast mode            ✓
──────────────
Settings                ▸  On-head detection ✓ · Smart pause ✓ · Auto-answer calls ✓ · Comfort call ✓
                           Touch controls locked ✓ · Auto power-off ▸ · Voice prompts ▸ · Sidetone ▸
                           Bluetooth mode ▸ (Best audio / Best compatibility)
Headset                 ▸  (only when >1 connected: radio list)
──────────────
Open window
Start with Windows      ✓
Quit
```

When no headset is connected, every headset section is hidden and only Open / Start with Windows / Quit remain.

## 6. Phases (goal-chaseable)

### Phase 0 — Feasibility spikes ✅ (2026-10-13)

- [x] `rust-toolchain.toml` pinned to `1.95-x86_64-pc-windows-msvc` (already installed; VS Build Tools 2022 present).
- [x] **Spike A, RFCOMM** (`crates/bluetooth/examples/spike_rfcomm.rs`): WinRT works from an unpackaged exe.
  - `BluetoothDevice::GetRfcommServicesForIdWithCacheModeAsync(GAIA3 UUID, Uncached)` found the service in ~350 ms.
  - `StreamSocket::ConnectWithProtectionLevelAsync` connected in ~250 ms while the headset was connected for audio.
  - Readings: fw **3.38.3** → **EQ Gen2**, model `M4AEBT Denim`, battery 100, codec AAC,
    ANC pairs `[1,2, 2,0, 3,0]`, `ANC_Status` = 0 (answers, so it exists), Gen1 EQ commands → error code 1.
  - The Winsock fallback is not needed.
- [x] **Spike B, tray + eframe** (`app/examples/spike_tray.rs --auto`), 7/7 checks pass:
  - The tray thread (own message loop) keeps updating while the window is hidden.
  - eframe 0.36's `App::logic()` keeps running while hidden whenever `request_repaint()` is called.
    This removes the Win32 `ShowWindow` workaround; `ViewportCommand::Visible` is enough.
  - Close and minimize hide to the tray; show/restore works; Quit while hidden exits cleanly.
- **Exit:** met. No fallbacks needed.

### Phase 1 — Protocol crate + `hsctl` + hardware verification ✅

- [x] `crates/protocol`:
  - framer/deframer (v1/3/4, resync, checksum)
  - id helpers
  - typed `Request` with deny-list
  - `decode` → `Update`
  - capabilities from FW gates
- [x] EQ Gen1 port: **byte-for-byte identical** to the official app's JS on 32 golden vectors produced by running
      that JS in Node (`re/tools/eq_gen1_vectors.mjs`). Gen2 encoder/decoder.
- [x] Unit tests incl. real captured M4 frames (16 protocol tests).
- [x] `tools/hsctl`: `scan`, `info`, `get`, `set`, `monitor`, `eq`, `engine`, `raw --unsafe`.
- [x] Hardware checklist answered → `PROTOCOL.md` §6:
  - Gen2 EQ; sidetone 0–4
  - notifications echo every change, and registration dumps current state
  - `ANC_Status` toggles, but its audible effect is unconfirmed → experimental "Off"
  - new notifications: sample rate `0x089A`, EQ band layout `0x108B/0x108D/0x108F/0x1091`

### Phase 2 — Engine ✅

- [x] `bluetooth::discovery` (`DeviceWatcher` with `IsConnected`) and `bluetooth::rfcomm` (SDP cached → uncached,
      connect watchdog, reader thread).
- [x] `engine::client`: one request in flight, response/error correlation, unsolicited forwarding, deny-list.
- [x] `engine::session`: init (identity → caps), notification registration, refresh, priority queue with
      slider coalescing, battery poll, timeout → link-dead detection.
- [x] `engine::Engine` actor: discovery → active device selection (explicit > current > preferred > first
      connected), reconnect backoff 1/3/5/10/30/60 s, action → request planning, published snapshots + notify callback.
- [x] `engine::sim`: simulated M4 that mirrors the observed hardware behaviour (`--sim`).
- [x] Integration test against the simulator (connect, sync, actions, coalescing, experimental off, reconnect).
- [x] Real hardware: `hsctl engine` connects in 0.5 s and is fully synced in under 1 s.

### Phase 3 — Tray-first app ✅

- [x] Single instance:
  - named mutex; a second launch signals the first to show its window
  - `--quit` asks the running instance to exit cleanly (tray icon removed)
- [x] Tray thread (own Win32 message loop, woken by an auto-reset event from the engine and UI).
- [x] Battery icon renderer (tiny-skia + Segoe UI Bold via ab_glyph):
  - exact `SM_CXSMICON` size for the system DPI
  - follows the taskbar's light/dark theme
  - states: disconnected, connecting, number + level bar, charging (green), low (red number)
- [x] Right-click menu rebuilt from state:
  - noise mode + transparency level
  - adaptive ANC, wind, EQ preset, bass boost, podcast
  - headset settings submenu, headset switcher
  - Bluetooth settings, Open, Settings…, Start with Windows, Quit
- [x] Close/minimize → tray; left-click toggles (brings a buried window forward instead of hiding it).
- [x] Settings in `%APPDATA%\SennheiserControl\settings.json` (was `HeadsetControl` before the rename); autostart via HKCU `Run`
      (round-trip test: `cargo test -p app -- --ignored`); `--minimized`.
- [x] Low-battery alert via `Shell_NotifyIconW(NIF_INFO)` on our icon. The uID is probed (it is 2, not 1).
      **Note:** toasts are disabled system-wide on the dev PC (`ToastEnabled=0`), so it was verified by API
      result only.

### Phase 4 — Window UI ✅

- [x] Theme tokens (dark/light, follows Windows; manual override), Segoe UI loaded from the system, Phosphor icons.
- [x] Widgets: card, segmented control (sliding pill), toggle, accent slider, battery ring, chips, setting rows,
      headphones illustration, spinner, buttons.
- [x] EQ editor: draggable handles, Catmull-Rom curve with gradient fill, dB grid, 0.5 dB steps (Shift = 0.1),
      double-click resets a band. Preset chips with auto-detection; save / right-click delete custom presets.
- [x] Screens:
  - header (battery ring, codec / sample rate chips, multi-device switcher with address tags)
  - noise control, sound, headset controls, about-this-headset
  - settings page
  - empty, connecting and retrying states
- [x] Borderless window, custom title bar, DWM rounded corners, resize grip, flyout positioning above the tray.
- [x] Sliders and EQ throttled to ~12 Hz, coalesced in the engine; local values held until the device echoes
      them (1.5 s grace); error toast.
- [x] Verified visually (dark + light, settings, multi-device, EQ curve) and against the real M4.
- [ ] Mica backdrop: skipped (opaque bg looks cleaner with egui; revisit if wanted).

### Phase 5 — Polish & ship ✅ (mostly)

- [x] App icon (generated by `--render-icons`, embedded via `app/app.rc` with version info).
- [x] Low-battery alert, about/disclaimer, file log (`app.log`, rotates at 2 MB), `--debug` frame tracing.
- [x] Release build: **9.1 MB** single exe.
  - Hidden in the tray and connected to the M4: **0.16 % of one core**, ~90 MB private memory
    (mostly the GL context).
- [x] Portable exe.
- [ ] Installer (MSI / Inno Setup): not done. The exe is self-contained; autostart is handled in-app.
- [ ] Possible memory diet: drop the GL context while hidden (would need a non-eframe window or a glow-less backend).

### Still needs the real headset / a human

- [ ] Does "Off" (`ANC_Status = 0`) audibly disable noise cancelling? (Experimental toggle in Settings.)
- [ ] Power-cycle / out-of-range → reconnect. The code path is covered by the simulator reconnect test and the
      watcher logic, but hasn't been exercised on hardware yet.
- [ ] Two real headsets at once (simulated with `HC_SIM_DEVICES=2`).
- [ ] EQ gen 1 on firmware < 3.18.

### Phase 6 — Stretch

- [ ] More GAIA v3 profiles: Accentum Plus and Accentum first (headband, closest to M4), then the earbuds
      (L/R/case battery UI, `PhysicalDevice_State` in case). They would be marked "experimental", with no hardware
      to test on.
- [ ] Multipoint management (paired-device list, connect/disconnect hosts).
- [ ] Sound personalization on/off (and maybe the hearing test).
- [ ] Global hotkey to cycle noise mode.

## 7. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Windows won't open the custom RFCOMM service while A2DP/HFP are active | Spike A first. Winsock `AF_BTH` fallback. If both fail, investigate the BLE GATT path (`GAIA3_BLE_*` UUIDs in `PROTOCOL.md` §1) |
| eframe hidden-window lifecycle quirks | tray on its own thread, Win32 `ShowWindow` via HWND (Spike B) |
| EQ Gen1 encoding is wrong and produces bad sound | EQ safe mode: read-back match before any write; flat EQ first; always keep a "Reset to flat" action |
| User's firmware is Gen2 (≥ 3.18.0) | Gen2 is simpler and already specified; selection is automatic |
| Phone app and this app both talking GAIA at once | test in Phase 1; on conflict, show "controlled by another device" and back off |
| Accidentally sending destructive commands | engine deny-list + typed API; raw frames only in `hsctl --unsafe` |
| Trademark / impersonation | own name and icon, no Sennheiser logos or imagery; "works with MOMENTUM 4" wording only |

# Sennheiser MOMENTUM 4 — control protocol map

Reverse-engineered from **Sennheiser Smart Control 4.9.3** (Android, `com.sennheiser.control`, versionCode 147651).
Everything below comes from **static analysis** of the shipped app. Nothing has been sent to a real headset yet.
Items marked **(verify)** are inferences that still need on-device confirmation.

| Artifact | SHA-256 |
|---|---|
| `com.sennheiser.control.apk` (base split, from APKPure XAPK) | `194be9f1d9fa8fde255c49e87cc6ab5c85d6e01a572fdd6c6053ed65bbe101de` |
| `Sennheiser Smart Control_4.9.3_APKPure.xapk` | `78d353b6d19a5102ef03076c7504264d156dc30329a900293dad82fd1911bc76` |

> Note: `uptodown-com.sennheiser.control.apk` in this folder is **not** Smart Control. It is the Uptodown store
> app (`package="com.uptodown"`) and contains no Sennheiser code. It can be deleted.

## 0. How the app is built (and why this was easy)

- Smart Control is a **NativeScript + Angular** app. All device logic is JavaScript in `assets/app/bundle.js`,
  not native code, so there was no need for Ghidra or JADX on the control path.
- Each product is described by a **declarative JSON device model** (`assets/app/<product>.json`) listing every
  property with its GAIA vendor ID, command/response/notification IDs and parameter types. Optional feature
  modules are merged from `assets/app/features/<name>.json`, gated by firmware version
  (loader: `bundle.pretty.js:20331-20387`).
- The M4 model is `m4.json` (`product_name: "Momentum 4"`, `protocol: "GAIAV3"`, article number `509266`) plus
  the features `user-eq-1`, `user-eq-2` (fw ≥ 3.18.0), `audio-mode-1` and `sound-personalization-1` (fw ≥ 2.12.0).

Kept in this repo:

```
re/device-model/m4.json                    # authoritative command table for the M4
re/device-model/features/*.json            # EQ / audio mode / sound personalization modules
re/tools/summarize_model.py                # dumps Get/Set/Event commands from a model JSON
re/tools/summarize_invoke.py               # dumps Invocation/Result-style commands
re/tools/eq_gen1_reference.py              # reference port of the EQ Gen1 math + frame builder
```

Not in the repo, because it is Sennheiser's code: `re/app-js/bundle.pretty.js`, which the `bundle.pretty.js:NNNN`
line references in this document point into. It is the official app's `assets/app/bundle.js` (APK opened with
JADX 1.5.1), pretty-printed with Prettier. `re/tools/eq_gen1_vectors.mjs` is also left out. It produced
`crates/protocol/tests/eq_gen1_vectors.json` by running the app's own EQ functions, and the generated vectors
are kept.

---

## 1. Transport

| Item | Value | Evidence |
|---|---|---|
| Physical link | **Bluetooth Classic, RFCOMM (SPP-style)** | M4 device config: `preferredTransportAndroid = CLASSIC`, `preferredTransportIOS = CLASSIC` (`:64936-64947`) |
| RFCOMM service UUID | **`a2129ff3-081b-4c45-8afe-469d9c4842ec`** (`GAIA3_SPP_UUID`) | `:42038`, M4 config `rfcommService` (`:64946`, `:66164`) |
| Protocol | Qualcomm **GAIA v3** with Sennheiser vendor extensions | `m4.json` `protocol: GAIAV3` |
| Min firmware for RFCOMM control | **2.11.26** (`rfcommMinVersion`) | `:64948`, `:66180` |
| BLE alternative (not preferred for M4) | service `63331337-23C1-11E5-B696-FEFF819CDC9F`, write `6333133B-…`, read/notify `6333133C-…`, data `6333133D-…` (`GAIA3_BLE_*`) | `:42036-42050`, `:64941-64945` |
| Authentication / handshake | **None found** on the GAIA channel | no auth or challenge commands in any model JSON |

Other UUIDs the app knows (not used for M4): legacy GAIA SPP `a2129ff3-…-469d9c4842eb`, Nautilus SPP
`1ddce62a-ecb1-4455-8153-0743c87aec9f`, BLE 16-bit services `0xFDCE` (Sennheiser) and `0xFCFE` (Sonova).

### Device identification

1. **Bluetooth name** is an exact-match lookup: `"MOMENTUM 4"` → M4. `"Mars"` (the internal codename) also maps
   to M4 (`:21714-21755`).
2. After connecting, the app reads **`Versions_ModelId`** (`0x0495:0x1206`), which returns a string such as `M4AEBT`,
   `M4AEBT Black`, `M4AEBT White`, `M4AEBT Denim`, `M4AEBT Graphite`, `M4AEBT JD`, `M4AEBT PRIDE`,
   `M4AEBT SE` or `M4AEBT Olive` (`:21286-21363`, `:3523-3540`).
3. The presence of the GAIA3 SPP UUID in the SDP record confirms a GAIA3-capable Sennheiser device.

---

## 2. Framing (RFCOMM byte stream)

From `SPPGaiaFramer` / `SPPGaiaDeframer` (`:17992-18145`). The M4 is created with framer version `3` (`:18163-18166`).

```
+------+---------+-------+--------+-----------+------------+-------------+-----------+
| 0xFF | version | flags | length | vendor_id | command_id |  payload    | [chk]     |
|  1B  |   1B    |  1B   | 1B/2B  |  2B (BE)  |  2B (BE)   | length B    | 1B opt.   |
+------+---------+-------+--------+-----------+------------+-------------+-----------+
```

- **version**: `0x03` for normal M4 control traffic. The deframer accepts `0x01`, `0x03` and `0x04`.
- **flags**: bit0 = XOR checksum present (the app never sets it); bit1 = 16-bit length (only with version 4).
  Any other flag bit causes the frame to be dropped.
- **length**: payload length only, **excluding** the 4 bytes of vendor_id and command_id. It is 1 byte unless the
  16-bit flag is set; the app only uses that for >254-byte payloads during OTA.
- All multi-byte fields are **big-endian** (`Gaia3Command.littleEndian = false`, `:73465`).
- No escaping/byte-stuffing. The receiver resyncs on `0xFF`.
- Version-4 / extended framing is only negotiated by the OTA updater, via `Core_SetTransportParameter(7, 4)`
  (`:33101-33119`). Control traffic does not need it.

Example: get battery → `FF 03 00 00 04 95 06 03`

### GAIA v3 command-ID layout

`command_id` (16 bits) = `feature (7 bits) | packet type (2 bits) | command (7 bits)`

| Type bits (`(id >> 7) & 3`) | Meaning | Example (battery level) |
|---|---|---|
| `0` | Command (host → device) | `0x0603` |
| `1` | **Notification** (unsolicited, device → host) | `0x0683` |
| `2` | Response | `0x0703` |
| `3` | **Error**: payload byte 0 is the error code (`(id & 0x180) == 0x180`, `:73467-73484`) | `0x0783` |

So `response = cmd | 0x0100`, `notification = cmd | 0x0080` and `error = cmd | 0x0180`.

### Vendor IDs

| Vendor | Use |
|---|---|
| `0x0495` | Sennheiser vendor features: nearly everything user-facing |
| `0x001D` | Qualcomm standard GAIA v3 features: core, voice UI, debug, DFU, statistics |

### Sennheiser (0x0495) feature IDs

| Feature ID | Name (app) | Command range |
|---|---|---|
| 0 | core | `0x00xx` |
| 1 | upgrade | `0x02xx` |
| 2 | device | `0x04xx` |
| 3 | battery / power | `0x06xx` |
| 4 | genericAudio | `0x08xx` |
| 8 | userEQ | `0x10xx` |
| 9 | versions | `0x12xx` |
| 10 | deviceManagement | `0x14xx` |
| 11 | mmi | `0x16xx` |
| 12 | transparentHearing | `0x18xx` |
| 13 | ANC | `0x1Axx` |
| 16 | personalizedSound | `0x20xx` |

---

## 3. Session flow used by the app

1. Open RFCOMM to `GAIA3_SPP_UUID`.
2. Read `Versions_ModelId` (`0x1206`) to identify the device variant (`:3523`).
3. Read the firmware version via **`Service_SystemReleaseVersion`** (`0x1201` → `UINT16 major, minor, patch`). This
   is the version used for all feature gating (`GaiaProduct.firmwareVersionKey`, `:3370`).
4. Register for notifications with `Config_RegisterNotification_Sennheiser` (`0x0495:0x0007`, payload =
   `UINT8 featureId`), one call per feature (`Momentum4Product.setupDevicePropertyEvents`, `:8273-8281`):
   `4` genericAudio, `10` deviceManagement, `12` transparentHearing, `13` ANC, `3` battery, `2` device, `11` mmi.
   With EQ Gen2 it also registers `8` userEQ (`:12439`). Feature `16` (personalized sound) is defined as well.
5. Refresh every property the UI shows (Get commands), then rely on notifications for changes.
   Properties the app subscribes to (`:8496-8498`): battery %, charging state, paired-device info,
   TransparentHearingMode, ANC, ANC_Transparency, voice-prompt language and mode, timers, codec, smart pause,
   sidetone, comfort call, on-head detection, auto-answer, physical state, voice assistant, BT compatibility
   mode, low latency, auto-lock.

The Qualcomm core init (`Core_Api_Version`, `Core_GetSupportedFeatures`, `Core_GetTransportInfo`) is only
run by the firmware-update service (`Gaia3FrameworkService`, `:33060-33123`). It is not needed for control.

---

## 4. Command reference (vendor `0x0495` unless noted)

Set responses carry no payload unless shown. "Ntf" = notification ID sent when the value changes on the device
(requires the feature's notification registration).

### 4.1 Noise control: ANC / transparency (feature 13 + 12)

| Property | Get → Rsp | Set | Ntf | Payload |
|---|---|---|---|---|
| **ANC** (mode/state pairs) | `0x1A01` → `0x1B01` | `0x1A00` | `0x1A81` | Get rsp: `UINT8 mode, UINT8 state` ×3. Set: `UINT8 mode, UINT8 state` |
| **ANC_Transparency** (level) | `0x1A03` → `0x1B03` | `0x1A02` | `0x1A83` | `UINT8 level` 0–100 |
| ANC_Status (on/off) | `0x1A05` → `0x1B05` | `0x1A04` | `0x1A85` | `UINT8` 0 = OFF, 1 = ON. **In the model, but the M4 UI never uses it (verify)** |
| **TransparentHearingMode** | `0x1801` → `0x1901` | `0x1800` | – | `UINT8` 1 = pause music at full transparency, 0 = keep playing |
| TransparentHearing_Status | `0x1805` → `0x1905` | `0x1804` | `0x1885` | `UINT8` on/off. In the model, unused by the M4 UI |
| TransparentHearing (TW variant) | `0x1803` → `0x1903` | `0x1802` | `0x1883` | `UINT8`. In the model, unused by the M4 UI |

ANC `mode` values (`:77150-77153`, `:76098-76104`, `:77187-77191`):

| mode | Meaning | state values |
|---|---|---|
| `1` | **Anti-wind** (wind noise reduction) | `0` OFF, `1` MAX, `2` AUTO |
| `3` | **Adaptive ANC** | `0` off, `1` on |
| `2` | **Comfort mode** (Smart Control Plus ANC mode enum: `antiWind = 1`, `comfortMode = 2`, `adaptive = 3`; the old app called it anti-pressure). Not exposed for the M4 | `0` off, `1` on |

How the M4 UI maps to commands (`Momentum4Product.sendFeatureToDevice`, `:8393-8403`):

- Adaptive ANC toggle → `ANC set [3, 0|1]`
- Wind noise reduction → `ANC set [1, 0|1|2]`
- Transparency slider (0–100) → `ANC_Transparency set [level]`
- "Pause music at full transparency" → `TransparentHearingMode set [0|1]`
- Phone-call sidetone ("phoneCallsValue") → `Sidetone_SetState`. The M4 UI sets the max to **4**, while the generic
  sidetone buttons are 0/1/2 = off/min/max (`:8191-8195`, `:3378-3390`). **(verify the real range)**

The official app has **no "ANC off"** for the M4. Noise control is adaptive on/off + anti-wind + transparency slider.

### 4.2 EQ, audio mode, sound personalization (feature 8, 4, 16)

#### Which EQ implementation applies (`MultiBandEqFactory`, `:12513-12550`)

| Firmware | Implementation | Feature file |
|---|---|---|
| < 2.12.0 | legacy 3-band EQ via knob X/Y (not decoded in detail) | user-eq-1 |
| ≥ 2.12.0 and < 3.18.0 | **Eq5BandGen1**: 5 bands rendered as a master gain + 4 high-shelf filters | user-eq-1 |
| ≥ 3.18.0 | **Eq5BandGen2**: device-side sliders | user-eq-2 |

> ⚠ Command `0x1002` means **different things** in Gen1 (`EQ_UserParameterGroup_Set`) and Gen2
> (`UserEQ_GetSliderGain`). Pick the implementation from the firmware version first.

#### EQ Gen1 commands (user-eq-1)

| Property | Command | Payload |
|---|---|---|
| Sound_SetCustomEQ | `0x1002` → `0x1102` (err `0x1182`) | `INT16[]` parameter list (see below) |
| Sound_GetCustomEQ | `0x1003` → `0x1103` (err `0x1183`) | req `INT16[]` query list → rsp `INT16[]` |
| Sound_LevelShift | get `0x1007` → `0x1107`; set `0x1006` | `INT8` = level shift in **dB × 10** |
| Sound_CustomEQKnobX | get `0x1005` → `0x1105`; set `0x1004` | get `[UINT16 index=0]` → `UINT16`; set `[UINT16 0, UINT16 value]` |
| Sound_CustomEQKnobY | get `0x1005` → `0x1105`; set `0x1004` | get `[UINT16 index=1]` → `UINT16`; set `[UINT16 1, UINT16 value]` |

**Gen1 algorithm** (`Eq5BandGen1`, `:12261-12423`; M4 constants `:70218-70241`):

M4 constants:

```
minGain -6 dB, maxGain +6 dB
bands (UI labels):   63Hz   250Hz   1000Hz   4000Hz   8000Hz
shelf freqs (Hz):     -      120     400      3000     6000
bassBoost add (dB):   5      5       0        0        0
loudnessWeights:   -26.683 -10.813 -4.773   -4.813   -5.993
maxHeadroom (dB):     5      5       4        3        2
Q (all shelves):   1/sqrt(2)
```

`send()` performs these steps:

1. `g = clip(uiGains, -6, +6)`. If bass boost is on, add `bassBoost[i]` to each band.
2. Loudness compensation: `s = -10·log10( Σ 10^((g[i] + loudnessWeights[i]) / 10) )`.
3. Clipping prevention: `r = max(0, max_i(g[i] + s − maxHeadroom[i]))`. Then `levelShift = round1(r > 0 ? s − r : s)`.
4. `x[i] = g[i] + levelShift`, `master = x[0]`, `shelf[k] = x[k+1] − x[k]` for k = 0..3.
   If `trunc(master·100) == 425`, the app forces `master = 4.24`.
5. Send `Sound_SetCustomEQ` with this INT16 list (big-endian, values truncated to int):

```
[17,                                   // number of (id, value) pairs
 0x0101, 60*master,                    // master gain, 1/60 dB
 0x0110, 11, 0x0111, 3*120,  0x0112, 60*shelf0, 0x0113, 4096*Q,   // band 1
 0x0120, 11, 0x0121, 3*400,  0x0122, 60*shelf1, 0x0123, 4096*Q,   // band 2
 0x0130, 11, 0x0131, 3*3000, 0x0132, 60*shelf2, 0x0133, 4096*Q,   // band 3
 0x0140, 11, 0x0141, 3*6000, 0x0142, 60*shelf3, 0x0143, 4096*Q,   // band 4
 0]                                    // trailing word, meaning unknown (verify)
```

   Parameter id `0x01Bx`: B = band (0 = master, 1–4 = filter), x = 0 type, 1 freq (×3 = 1/3 Hz),
   2 gain (×60 = 1/60 dB), 3 Q (×4096 = Q12). Filter type **11** is inferred to be Qualcomm's "2nd-order high
   shelf" from the Qualcomm user-EQ enum, not from the APK **(verify)**. The math (master + cascaded shelf
   steps) is consistent with that.
6. Send `Sound_SetLevelShift` = `round(levelShift·10)` as INT8.
7. Send `KnobY` = `0x4000 | (bassBoost ? 0x1000 : 0)`, which encodes "new storage" + bass-boost flag
   (`EqKnobValuesCoder`, `:43912-43977`). KnobX is not written by `send()`.

**Reading back** (`refresh()`, `:12286-12308`):

- `LevelShift` get gives `levelShift = v/10`.
- `KnobY` get: bit `0x1000` = bass boost.
- `Sound_GetCustomEQ([5, 0x0101,0, 0x0112,0, 0x0122,0, 0x0132,0, 0x0142,0])` returns `[5, id,val, id,val, …]`.
  Then `master = rsp[2]/60` and `shelf[k] = rsp[4+2k]/60`.
- `Sound_GetCustomEQ([4, 0x0110,0, 0x0120,0, 0x0130,0, 0x0140,0])` returns filter types. If all are 0 the EQ is
  unconfigured; if bands 3 and 4 are 0 they are bypassed. In both cases the app re-sends a flat EQ.
- UI gains: `ui[0] = master`, `ui[k+1] = ui[k] + shelf[k]`. Subtract bassBoost if it is on, then subtract levelShift
  and round to 0.1 dB.

Worked frames (computed by `re/tools/eq_gen1_reference.py`):

```
Flat EQ, no bass boost:
  FF 03 00 48 04 95 10 02  00 11 01 01 00 00 01 10 00 0b 01 11 01 68 01 12 00 00 01 13 0b 50
                           01 20 00 0b 01 21 04 b0 01 22 00 00 01 23 0b 50 01 30 00 0b 01 31 23 28
                           01 32 00 00 01 33 0b 50 01 40 00 0b 01 41 46 50 01 42 00 00 01 43 0b 50 00 00
  FF 03 00 01 04 95 10 06  00            (level shift 0)
  FF 03 00 04 04 95 10 04  00 01 40 00   (knob Y: new-storage, bass boost off)
"Dance" preset [3.5, 2, -1.5, 1.5, 3]: levelShift -1.3 dB, master 2.2, shelves [-1.5, -3.5, 3.0, 1.5]
```

#### EQ Gen2 commands (user-eq-2, fw ≥ 3.18.0) (`Eq5BandGen2`, `:12428-12510`)

| Property | Command | Payload |
|---|---|---|
| UserEQ_GetConfiguration | `0x1000` → `0x1100` | rsp `UINT8 bands, INT8 minRange, INT8 maxRange, UINT8 selectablePreset, UINT8 userPresets`. Ranges are in dB×10 |
| UserEQ_SetSliderGain | `0x1001` → `0x1101` (ntf `0x1082`) | `UINT8 band, INT8 gain` (dB×10, "centibel") |
| UserEQ_GetSliderGain | `0x1002` → `0x1102` | `UINT8 band` → `INT8 gain` |
| User_EQ_SubMode (= bass boost) | get `0x1009` → `0x1109`, set `0x1008`, ntf `0x1089` | `UINT8` 1 = bass boost on |
| Slider gain notification | ntf `0x1082` | `INT8 gain[]` (5) |

M4 Gen2 UI range: ±6 dB, labels 63/250/1000/4000/8000 Hz (`:70290-70309`). The device reports its own range.
`send()` writes the sub-mode, then only the bands that changed.

#### EQ presets the app ships for M4 (`:12600-12630`, applied client-side as UI gains)

| Preset | 63 | 250 | 1k | 4k | 8k |
|---|---|---|---|---|---|
| Rock | 0 | 2 | 2.5 | 1.5 | -2 |
| Pop | 0 | -2.5 | 0 | 2.5 | 0 |
| Dance | 3.5 | 2 | -1.5 | 1.5 | 3 |
| Hip-Hop | 3 | 1.5 | -1.5 | 0 | -1.5 |
| Classical | -2 | -1.5 | 0 | 3.5 | 4 |
| Movie | 0 | 0 | 2 | 2 | -2 |
| Jazz | -3.2 | 0 | 2.2 | 2.2 | 0 |

#### Audio mode (audio-mode-1)

| Property | Get → Rsp | Set | Ntf | Payload |
|---|---|---|---|---|
| AudioMode | `0x0804` → `0x0904` | `0x0803` | `0x0884` | `UINT16`. Old model: `1` USEREQ, `2` SPEECHMODE, `3` FLAT. Smart Control Plus (`Lgb` enum): `0` off, `1` userEq, `2` podcastMode, `3` personalizedSound, `4` parametricEq, `5` hearingEnhancement |

**The user EQ is only heard in audio mode 1.** Firmware 3.38 starts in mode `0` (EQ off, the headset's own
tuning, which Smart Control Plus calls "Neutral"). In that mode the slider gains are stored and acknowledged, but
not applied. That's why the first builds' EQ changed nothing. This app sends the gains first, then
`AudioMode = 1`. Its "Neutral" preset sets mode `0`. Turning podcast mode off restores the mode that was active
before, as Smart Control Plus does.

#### Sound personalization (sound-personalization-1, fw ≥ 2.12.0)

| Property | Get → Rsp | Set | Ntf | Payload |
|---|---|---|---|---|
| PersonalizedSound_Mode | `0x2001` → `0x2101` | `0x2000` | `0x2081` | `UINT8`: 0 not_parameterized, 1 disabled, 2 standby, 3 enabled, 4 calibrating |
| PersonalizedSound_Threshold | `0x2003` → `0x2103` | `0x2002` | – | `REAL32` ×6 |
| PersonalizedSound_Ratio | `0x2005` → `0x2105` | `0x2004` | – | `REAL32` ×6 |
| PersonalizedSound_Gains | `0x2007` → `0x2107` | `0x2006` | – | `REAL32` ×3, `UINT8 presetX`, `UINT8 presetY` |
| PersonalizedSound_Timeout | – | – | `0x2088` | – |
| PersonalizedSound_Reset | `0x2009` → `0x2109` | | | – |

The hearing test itself (tones and thresholds) runs in the app (`assets/app/audio/fit_test.*`). Those UI flows
are not mapped here.

### 4.3 Battery / power (feature 3)

| Property | Get → Rsp | Set | Ntf | Payload |
|---|---|---|---|---|
| **Battery_Percent** | `0x0603` → `0x0703` | – | `0x0683` | `UINT8[]`. M4 sends one value = %, `255` = unknown |
| **Battery_ChargingStatus** | `0x0602` → `0x0702` | – | `0x0682` | `UINT8[]`: 0 DISCONNECTED, 1 NORMAL_CHARGING, 2 CHARGING_COMPLETE |
| Config_GetTimer | `0x0601` → `0x0701` | | | req `UINT8 timerId` → `UINT8 timerId, UINT16 seconds` |
| Config_SetTimer | `0x0600` → `0x0700` | | | `UINT8 timerId, UINT16 seconds` |

Timer IDs: `0` POWER_OFF_TIMER (**auto power-off**; UI choices 0 = never, 900, 1800, 3600 s, `:76164-76186`),
`1` LIMBO_TIMEOUT. The auto power-off UI is only shown when on-head detection is supported (fw ≥ 2.6.3), and it
shows 0 when on-head detection is off (`:8304`).

### 4.4 Generic audio / behaviour settings (feature 4)

| Property | Get → Rsp | Set | Ntf | Values | Min FW |
|---|---|---|---|---|---|
| Sound_CodecUsed | `0x0800` → `0x0900` | – | `0x0880` | 0 SBC, 1 AAC, 2 aptX, 3 aptX-LL, 4 MP3, 5 aptX-HD, 6 Faststream, 7 LHDC, 8 aptX Adaptive, 9 aptX Lossless, 10 LC3, 255 none | |
| Setting_AudioPromptMode | `0x0802` → `0x0902` | `0x0801` | – | 0 tones+voice off, 1 tones only, 2 tones+voice | 2.6.12 |
| SidetoneState | `0x0806` → `0x0906` | `0x0805` | – | `UINT8` level (see 4.1) | |
| VoicePrompt_Language | `0x0807` → `0x0907` | *(no Set in model)* | – | 0 EN, 1 DE, 2 FR, 3 ES, 4 ZH, 5 JA, 6 RU, 7 KO | 2.6.12 |
| AudioPrompt_GetAvailableAudioPromptsLanguages | `0x0808` → `0x0908` | | | `UINT8[]` | |
| AudioPrompt_GetAudioPromptsLanguageVersions | `0x0809` → `0x0909` | | | req `UINT8 side` | |
| Setting_AutoCall (auto-answer calls) | `0x080B` → `0x090B` | `0x080A` | – | 0/1. Effective only with on-head detection on | 2.13.28 |
| Setting_SmartPause | `0x080D` → `0x090D` | `0x080C` | – | 0/1. Effective only with on-head detection on | 2.6.3 |
| Setting_ComfortCall | `0x0815` → `0x0915` | `0x0814` | – | 0/1 | |
| LowLatencyMode | `0x0818` → `0x0918` | `0x0817` | `0x0898` | 0/1. **Hidden in app (min fw 99.99.99)** | – |

### 4.5 Device (feature 2)

| Property | Get → Rsp | Set | Ntf | Values | Min FW |
|---|---|---|---|---|---|
| Setting_OnHeadDetection | `0x0401` → `0x0501` | `0x0400` | – | 0/1. The app re-reads smart pause, auto-call and timer 0 after changing it | 2.6.3 |
| PhysicalDevice_State | `0x0402` → `0x0502` | – | `0x0482` | `UINT8` 0 UNKNOWN, 1 IN_CASE, 2 NOT_ON_HEAD, 3 ON_HEAD (2nd byte for right earbud, unused on M4) | 2.12.0 |
| BluetoothCompatibilityMode | `0x0406` → `0x0506` | `0x0405` | – | 0 betterAudio, 1 betterCompatibility | 2.12.0 |

### 4.6 Versions / identity (feature 9 + Qualcomm core)

| Property | Command | Response |
|---|---|---|
| Service_HWRevision | `0x1200` → `0x1300` | `UINT16 hwRevision, UINT16 junk` |
| **Service_SystemReleaseVersion** | `0x1201` → `0x1301` | `UINT16 major, minor, patch`. **This is the firmware version** |
| FirmwareVersions | `0x1202` → `0x1302` | `UINT8[6]` component versions |
| Versions_ModelId | `0x1206` → `0x1306` | string, e.g. `M4AEBT Black` |
| Core_SerialNumber | **`0x001D:0x0003`** → `0x0103` | string |
| Core_Api_Version | `0x001D:0x0000` → `0x0100` | `UINT8 major, minor` (GAIA API) |
| Core_GetSupportedFeatures(+Next) | `0x001D:0x0001` / `0x0002` | `UINT8 moreData, (featureId, version)[]` |
| Development_Primary_Side | `0x0041` → `0x0141` | 0 BOTH, 1 LEFT, 2 RIGHT |

### 4.7 Touch controls (feature 11)

| Property | Command | Payload |
|---|---|---|
| **Setting_AutoLockMode** (= "touch controls disabled") | get `0x1607` → `0x1707`, set `0x1606`, ntf `0x1687` | 0 OFF (touch active), 1 ON (touch locked) |
| GetMMIConfig | `0x1601` → `0x1701` | `UINT8 button, UINT8 pattern` → `button, pattern, function` |
| SetMMIConfig | `0x1600` → `0x1700` | `UINT8 button, UINT8 pattern, UINT8 function` |
| SetDefaultMMIConfig / IsDefaultMMIConfig | `0x1604` / `0x1605` | – / `BOOL` |

On the M4 the gesture map is **read-only** in the app: right pad slide up/down = volume, zoom/pinch = transparency
up/down, slide back/front = previous/next track or call handling, and so on (`:76466-76550`). The only M4 control
setting is the touch lock. The MMI function codes in the model are 0 none, 1 play/pause, 2 next, 3 prev, 4 vol+,
5 vol−, 6 transparent hearing, 7 voice assistant, 0x80 accept, 0x81 reject, 0xF0–0xFF custom. They are written
for earbuds.

### 4.8 Paired-device / multipoint management (feature 10)

| Property | Command | Payload |
|---|---|---|
| PairedDevicesListSize | `0x1400` → `0x1500` | → `UINT16 count` |
| PairedDevicesGetDeviceInfo | `0x1401` → `0x1501` | `UINT8 idx` → `idx, priority, connectionStatus, STRING name` |
| PairedDevicesConnectDevice | `0x1402` → `0x1502` | `UINT8 idx` |
| PairedDevicesDisconnectDevice | `0x1403` → `0x1503` | `UINT8 idx` |
| PairedDevicesGetConnectionStatus | `0x1404` → `0x1504` (ntf `0x1484`) | `UINT8 idx` → `idx, status` |
| PairedDevicesDeleteEntry | `0x1405` → `0x1505` | `UINT8 idx` |
| PairedDevicesDeleteList | `0x1406` → `0x1506` | – (clears the whole pairing list) |
| PairedDevicesOwnDeviceIndex | `0x1407` → `0x1507` | → `UINT8 idx` (the controlling host) |
| PairedDevicesMaxBTConnections | `0x1409` → `0x1509` | → `UINT8` |

The list starts at index 0 (`pairedDevicesManagementStartIndex`). `pairedDevicesDisconnectOwnDevice: true`.

### 4.9 Dangerous / do-not-touch

| Command | Notes |
|---|---|
| `Service_FactoryReset` `0x0040` → `0x0140` | Factory reset. The app also deletes the bonding and resets its overlay |
| `Upgrade_*` (`0x0200`, `0x0201`, ntf `0x0282`; Qualcomm DFU `0x001D:0x0C00…`, tunnelled upgrade ops) | Firmware OTA. Out of scope |
| `Debug_*` (`0x001D:0x0803/0x0804`), `Core_DataTransfer*` (`0x0009`/`0x000A`) | Panic-log retrieval |
| `Statistics_*` (`0x001D:0x1800–0x1802`) | Usage stats (fw ≥ 2.12.0) |
| `VoiceUI_*` (`0x001D:0x0600–0x0602`) | Alexa selection. Hidden in the app (min fw 99.99.99) |

---

## 5. Firmware gating summary (`m4.json` → `parameters`)

| Gate | Version |
|---|---|
| RFCOMM control at all | 2.11.26 |
| On-head detection, smart pause, auto power-off | 2.6.3 |
| Voice prompts | 2.6.12 |
| Customizable controls (touch lock UI), BT compatibility mode, physical state, 5-band EQ Gen1, statistics, sound personalization (Smart Control Plus: Hi-Res and sound personalization from 2.13.18) | 2.12.0 |
| Auto-answer calls | 2.13.28 |
| EQ Gen2 (user-eq-2) | 3.18.0 |
| Voice assistant, low-latency mode, rename device | 99.99.99 (disabled) |

---

## 6. Hardware verification (MOMENTUM 4 `M4AEBT Denim`, fw 3.38.3, 2026-10-13)

Checked with `tools/hsctl`. Every change below was reverted right after the test.

| Question | Answer |
|---|---|
| Does Windows open the `…42ec` RFCOMM service while A2DP/HFP are active? | **Yes.** SDP lookup ~350 ms, connect ~250 ms, from an unpackaged exe through WinRT `StreamSocket`. |
| EQ generation | fw 3.38.3 ≥ 3.18.0 → **Gen2**. Gen1 commands (`0x1005`, `0x1007`) return error 1. `UserEQ_GetConfiguration` = 5 bands, −6.0…+6.0 dB. |
| Gen2 slider write | `SetSliderGain [0, 30]` → ack, then notification `0x1082 [30,0,0,0,0]`. |
| ANC pairs | `[(1,2),(2,0),(3,0)]`: anti-wind AUTO, comfort mode (2) off, adaptive off. |
| `ANC_Status` (`0x1A04`/`0x1A05`) | Accepts set 0/1 and reads back. Default on this unit is **0**, even though noise cancelling was active. **Audible effect not yet confirmed**, so it's exposed as an experimental "Off" mode. |
| Sidetone range | **0–4** accepted; 5 → error 5. |
| AudioMode | Default reads **0** = EQ off (see 4.2). Set 2 (podcast) → notification `0x0884 [00 02]`; set 0 → accepted. |
| Limbo timer (`0x0601 [1]`) | error 5 (not supported). Auto power-off (timer 0) = 900 s. |
| Notifications | `Config_RegisterNotification_Sennheiser [feature]` makes the headset **immediately push the current value** of every property in that feature. Every set is then echoed as a notification. |
| Commands before registration | Work fine. Registration only enables push. |
| Phone connected at the same time | Paired-device list shows iPhone + this PC both connected (multipoint). Control from Windows works regardless. |

### Notifications not in the app's model (fw 3.38.3)

| Id | Payload example | Interpretation |
|---|---|---|
| `0x089A` | `00 00 BB 80` | **Sample rate**, UINT32 Hz (48000) |
| `0x108B` | `00 00 5A 01 01 45 02 05 DC 03 19 64 04 19 64` | Gen2 EQ band frequencies: `(band, UINT16 Hz)` ×5 → 90, 325, 1500, 6500, 6500 |
| `0x108D` | `00 08 00 01 0B 5C …` | Gen2 EQ Q factors: `(band, UINT16 Q12)` → 0.5, 0.71… |
| `0x108F` | `00 0E 01 0E …` | Gen2 EQ filter types: `(band, UINT8)` → 14 |
| `0x1091` | `00 00 00 01 00 00 …` | per-band `(band, INT16)`, probably the DSP gain |
| `0x1093` | `00 00` | unknown |
| `0x1885` | `00` | TransparentHearing_Status |

## 6b. Firmware ≥ 3.37: ANC master switch (Smart Control Plus)

Sennheiser moved the M4 to **Smart Control Plus** (`com.sonova.chb.control`; analysed 1.6.3, versionCode 72844).
It is a Flutter app: obfuscated Dart AOT plus encrypted assets (`SHASSET3` per-file nonce + XOR keystream).
blutter recovered its object pool:
- `NoiseControl2Mode = { anc, transparency, off, adaptive, custom }`
- simplified `{ anc, transparency, off }`
- on/off enum `{ off, on }`

Firmware 3.37.x added an "ANC off switch" that only this app exposes. On the wire it is
**`ANC_Status` (`0x1A04` set / `0x1A05` get / `0x1A85` ntf)**, the same master switch the old app used for the
Accentum family.

The decrypted `products/m4` config (see 6c) gives the M4 `NoiseControl` variant 2 with modes
**`[adaptive, custom]`**, plus **`off` from fw 2.13.32**. Verified on fw 3.38.3:

| Mode | Commands |
|---|---|
| Adaptive | `ANC_Status = 1`, ANC mode pair `(3, 1)` |
| Custom | `ANC_Status = 1`, ANC mode pair `(3, 0)`, `ANC_Transparency = level` (0 = max noise cancelling, 50 = neutral, 100 = max transparency) |
| Off | `ANC_Status = 0` |

While adaptive is on, the headset ignores `ANC_Transparency`. With `ANC_Status = 0`, changing only
`ANC_Transparency` has no audible effect either. That is why the first build's mode switch looked dead on a headset
left in "Off".

**Custom slider mapping (resolved from the Smart Control Plus code).** The app keeps the Custom value as a double
`v` in 0.0–1.0, defaulting to 0.5. Its GAIA codec decodes `v = ANC_Transparency / 100` (Dart `0x8d401c`) and
encodes `ANC_Transparency = round(v × 100)` as one byte (Dart `0x8d407c`). The slider label code (Dart `0xc2f39c`)
treats 0.5 as a neutral centre. Below it the label shows noise cancelling at strength `(0.5 − v) × 2`; above it,
transparency at `(v − 0.5) × 2`. So on the wire, 0 is maximum noise cancelling, 50 is neutral, and 100 is maximum
transparency. This app's Custom slider uses the same scale and snaps to the centre.

## 6c. Smart Control Plus feature set for the M4 (`products/m4/0`, decrypted asset)

Per-firmware `feature_variant_configs` (later keys override earlier ones):

| From fw | Features |
|---|---|
| 0.0.1 | Battery, ConnectionManagement (`multipointSwitchSupported: false`), FactoryReset, FirmwareUpdate, ProductInfo, AutoPowerOff, Codec, ComfortCalls, Crashdumps, Equalizer v1 (3 bands), NoiseControl v2 (`adaptive, custom`; anti-wind `off, max, auto`), PodcastMode, Sidetone (5 steps), SmartPause, SoundZones; app features FindHeadphones, Onboarding |
| 2.8.5 | OnHeadDetection, ToneAndVoicePrompts (`volumeSliderSupport: full`) |
| 2.12.0 | Telemetry |
| 2.13.18 | Classic (RFCOMM) transport; AudioModes (`highResolution`, `aptX96kHz`); AutoAnswerCalls; Codec list; Equalizer 5 bands + presets; SoundPersonalization (`usageWithHighResolutionSupported: false`); TouchControls (fixed map) |
| 2.13.32 | NoiseControl adds `off`; TouchControls `keepsMmiLockedAfterReboot: true` |
| 3.29.0 | ConnectionManagement `multipointSwitchSupported: true`; Equalizer v2 (graphic EQ, no parametric) |
| 3.31.0 | RenameDevice (beta, max 16 chars) |

Not in the Smart Control Plus M4 config, so this app dropped them: transparent-hearing "pause audio at full
transparency" (`0x1806/0x1807`) and the "Bluetooth mode" wording. Smart Control Plus renames that control (next
paragraph).

**Hi-Res 24 bit/96 kHz = `BluetoothCompatibilityMode` (`0x0405` / `0x0406`).** The Smart Control Plus decoder for
`AptX96kHzSupportMode` (Dart `0x8d66e8`) maps wire `0` → `on` and `1` → `off`. That matches the old model's
`betterAudio = 0`, `betterCompatibility = 1`. Both apps also describe the trade-off the same way: better codecs
versus a less stable connection. This is inferred from matching encoding and semantics; the command table entry
that feeds the decoder is reached through a dispatch table and was not traced. The app asks for confirmation
before enabling Hi-Res, as Smart Control Plus does. It also disables sound personalization while Hi-Res is on.

**Touch map** (`TouchControls` config, decoded with the app's gesture enum `mEa` and action enum `kEa`):

| Context | Gesture → action |
|---|---|
| Always | swipe up → volume up · swipe down → volume down · two-finger stretch → increase transparency · two-finger pinch → reduce transparency |
| Music | tap → play/pause · double tap → transparent hearing · swipe forward → next · swipe backward → previous · swipe forward + hold → fast forward · swipe backward + hold → rewind |
| Calls | swipe forward → accept · tap + hold → reject · swipe backward → end · tap → accept second call · double tap → toggle between calls |

Gesture ids: 1 tap, 2 doubleTap, 7 tapHold, 13 twoFingerStretch, 14 twoFingerPinch, 15 swipeForward,
16 swipeBackward, 17 swipeUpward, 18 swipeDownward, 19 swipeForwardAndHold, 20 swipeBackwardAndHold. Action
ids: 1–6 as in 4.7, 0x80 acceptCall, 0x81 rejectCall, 0x100 increaseTransparency, 0x101 reduceTransparency,
0x102 acceptSecondCall, 0x103 endCall, 0x104 toggleBetweenCalls, 0x105 fastForward, 0x106 rewind.

**Noise-mode command order** (Smart Control Plus `Xna` mode setters, Dart `0x136a788` / `0x136a8e0` /
`0x136a974`):
- Adaptive: `ANC_Status = 1`, then `(3, 1)`.
- Custom: `(3, 0)`, then `ANC_Status = 1`. It does not resend the level, because the headset keeps it.
- Off: `(3, 0)`, then `ANC_Status = 0`.

**Connection management** on the M4 is `ConnectionManagementByIndexGaia3FeatureVariant`. It covers the paired list
(4.8), connect, disconnect, and **delete device**, which is `PairedDevicesDeleteEntry` `0x1405 [idx]`. This app
offers "Forget device" only for a host that is disconnected and isn't this PC. The command is deny-listed for raw
use and only goes out after the user confirms. Afterwards the app re-reads the own index and the list, because
the indices shift.

**Battery:** Smart Control Plus re-reads the battery every 60 s while it is in the foreground ("Set up periodic
battery updates", `Duration(60 s)`). The old app only read it on connect and then relied on notifications. This
app polls battery and charging state every 60 s while connected.

Not implemented, because the write format is unverified: the multipoint on/off switch, rename, voice-prompt
volume, and voice-prompt language. The M4 model defines a get for the language, but no set. SoundZones,
FindHeadphones, firmware update and the hearing-test flow need a phone or are out of scope, so they are not
implemented either.

## 7. Handy frames

```
Get firmware version        FF 03 00 00 04 95 12 01
Get model id                FF 03 00 00 04 95 12 06
Get battery %               FF 03 00 00 04 95 06 03
Get charging state          FF 03 00 00 04 95 06 02
Get ANC (mode/state pairs)  FF 03 00 00 04 95 1A 01
Adaptive ANC on             FF 03 00 02 04 95 1A 00 03 01
Anti-wind AUTO              FF 03 00 02 04 95 1A 00 01 02
Transparency = 50           FF 03 00 01 04 95 1A 02 32
Get transparency            FF 03 00 00 04 95 1A 03
Register ANC notifications  FF 03 00 01 04 95 00 07 0D
Register battery ntf        FF 03 00 01 04 95 00 07 03
Get codec                   FF 03 00 00 04 95 08 00
Audio mode = Podcast        FF 03 00 02 04 95 08 03 00 02
Touch lock on               FF 03 00 01 04 95 16 06 01
```

# G HUB feature inventory

Built from G HUB's agent API (556 endpoints in `lghub_agent.exe`), its UI bundle
(`app.asar`: routes, panels, tabs) and the device features your hardware reports.
Status: **done** (works, verified on hardware) · **built** (implemented, not verified on
hardware) · **next** · **planned** · **n/a** (doesn't apply to your devices or to Linux).

## Devices and service

| G HUB feature | Status | Notes |
|---|---|---|
| Device discovery: USB, LIGHTSPEED receivers, headset dongle | done | Including the G522 behind its headset bridge |
| Stable identity across replug | done | USB port + product id |
| Battery status (`unified battery`) | done | Keyboard, mouse |
| Headset battery (centurion `0x0104`) | next | |
| Background agent that keeps settings applied | done | `lhubd`, systemd user unit |
| Settings re-applied on reconnect/wake | built | Rescan every 3 s |
| Multiple devices of the same model | built | Ids are per USB port |
| Device pictures | done | G HUB pictures imported at runtime; G502 X PLUS from the public product photo |
| Firmware update (DFU) | n/a | Excluded on purpose |
| Receiver pairing / unpairing | planned | `/devices/pairable`, `0x1894` |

## Lighting

| G HUB feature | Status | Notes |
|---|---|---|
| Fixed / breathing / color cycle (device effects) | done | Effect index looked up by id per device |
| Brightness | done | Scaled to each device's maximum; applied to live effects too |
| Per-key painting (freestyle) | done | Painted on the device picture in the Lighting page |
| Animated host effects | done | Wave, starlight, pulse (live), visualizer |
| Firmware reactive ripple (G515 `0x0017`) | done | Verified by user |
| Other firmware all-zones effects (G515 `0x000c 0x0013 0x0016 0x0018 0x0019`, G502 `0x0004 0x000c 0x000f 0x0010 0x0013`) | next | Parameters partly seen in captures |
| Audio visualizer | built | PipeWire monitor of default output |
| Screen sampler | planned | Wayland: needs xdg-desktop-portal screencast |
| LIGHTSYNC: same effect on all devices | done | Sync skips device-specific effects and devices running one |
| Palettes (vibrant, pastel, mono, signature, custom) | done | Own palettes plus saved custom colors in the color picker |
| Custom effect editor, save/import/export | planned | |
| Turn off lighting after inactivity | next | RGB `0x8071` fn 7 power settings seen in captures |
| Lighting off while the PC sleeps / locks | built | Locked session (logind LockedHint) → lights off, restored on unlock; Settings toggle, on by default |
| Low-battery and power-saving lighting effects | planned | Device settings `LOW_BATTERY_EFFECT`, `POWER_SAVING_EFFECT` |
| Saved custom effects and presets (save, delete, import, export) | planned | `/lighting/effect/presets/*`, `/lighting/effect/import|export` |
| Games driving lighting (LED SDK) | planned | `integration_manager_settings.allowLedSdk`; Proton games would need a bridge DLL |
| Mouse zone layout | built | Product photo with 8 strip regions; zone order not yet checked on the mouse |
| LampArray (Windows Dynamic Lighting) | n/a | Windows-only |

## Mouse (G502 X PLUS)

| G HUB feature | Status | Notes |
|---|---|---|
| DPI levels, default, shift | built | `0x2201` levels + active level in service and UI; shift = lowest level while held |
| Report rate | done | `0x8060` read/write verified |
| Default DPI level, DPI shift target, "go to shift" assignment | planned | `/mouse/dpi_default`, `/mouse/dpi_go_to_shift` |
| Advanced DPI (separate X/Y), DPI correction | planned | `DPI_ADVANCED`, `DPI_CORRECTION` |
| DPI tester tool | planned | G HUB ships `dpi-tester.min.js` |
| Left-handed (swap buttons) | planned | App setting `mouse_buttons_swapped` |
| DPI cycle up/down via mouse buttons | built | Button spy `0x8110`, masks in the driver file; not yet pressed on the mouse |
| Hi-res wheel / wheel mode | next | `0x2121` |
| Button assignments | built | Host-side via button spy + routing (as G HUB): keys, shortcuts, macros, mouse buttons, scroll, DPI, programs; buttons 7–9 still unnamed |
| Onboard memory mode | planned | Read back before any write |

## Keyboard (G515 LS TKL)

| G HUB feature | Status | Notes |
|---|---|---|
| Key assignments, G-shift | planned | `0x1b05` full key customization |
| Game mode (disable Win key etc.) | next | `0x4523` at index 0x0c; G HUB traffic seen (fn1 → 04, fn3 → 01, fn0 write) but needs a labeled capture to decode |
| Fn-layer and G-shift-layer key customization (remap, macros, samples, system actions; press/hold/toggle/loop playback) | planned | G HUB "FKC" |
| Onboard profiles | planned | `0x8101` |
| Per-key lighting | built | |

## Headset (G522)

| G HUB feature | Status | Notes |
|---|---|---|
| Sidetone | next | centurion `0x0604` |
| Equalizer (parametric), presets | next | `0x020d`, `centurion_eq_presets.json` |
| Mic processing (Logivoice: NR, gate, compressor, de-esser, de-popper, limiter, HPF, EQ) | next | `0x0900`–`0x0908`, runs on the headset |
| Mic gain, mute | next | `0x0611`, `0x0601` |
| Auto sleep, battery saver, tones | next | `0x0108`, `0x0618`, `0x060b` |
| Battery level | built | centurion `0x0104` fn0 byte 0 (seen draining); charging byte unconfirmed |
| Headset lighting | next | `0x0620`–`0x0622` |

## Microphone (Yeti GX)

| G HUB feature | Status | Notes |
|---|---|---|
| Gain, mute | done | Mute state read back from the mic |
| Lighting | done | |
| Blue VO!CE 2 engine | built | Real DTS ClearVoice 2 DLL via our PE loader, as a PipeWire virtual mic (`lhub-voice`) |
| VO!CE presets → engine parameters | done | Every preset field measured (docs/voice.md); the 8 G HUB presets load |
| VO!CE as a PipeWire virtual mic | built | "Yeti GX (Blue VO!CE)"; preset picker in the app; not yet heard by the user |
| Smart lock | planned | G HUB byte `0xae` vs ours `0xca` unresolved |
| Blue VO!CE automatic gain control, reference gain | next | Part of every VO!CE preset |
| Voice effects (21 presets: delay, flanger/phaser, reverb, ring modulation, pitch/formant voice changer) | planned | `AUDIO_EFFECTS_SETTINGS` cards for `yeti-gx`; engine is the DTS Studio APO in `driver_audio_apo` |

## Profiles, assignments, macros

| G HUB feature | Status | Notes |
|---|---|---|
| Profiles per application, auto switching | planned | Active-window tracking on KDE Wayland via KWin |
| Persistent features (settings shared across profiles) | planned | |
| Macros (record, edit, assign) | built | Key/shortcut steps with pauses on mouse buttons; keyboard keys still planned |
| Lua scripting | planned | |
| Import/export profiles | planned | |
| Sound samples (play audio clips from a key or button) | planned | `audio_samples` depot, `Generic_Sample_*`, `Expression_Sample_*` |
| System actions library (open apps, shortcuts, media) | planned | Built-in `MACRO_PLAYBACK` cards |
| Settings backup and restore | planned | `/local_settings_backup/*` |

## Not applicable

Analog keyboard features (actuation point, rapid trigger, key priority), wheels and
force feedback, Trueforce, haptics, LampArray, UVC cameras, Overwolf/Discord/OBS applets,
G HUB accounts/community/marketplace, game library (`gl`), Windows audio mixing (`mixline`).

## Windows drivers G HUB installs

| Package | What it is | Linux equivalent |
|---|---|---|
| `driver_audio_apo` | Audio effect plugins: Blue VO!CE 2 (DTS ClearVoice 2), Blue VO!CE 1 (DTS Studio), headset capture/render, speaker render | VO!CE 2 runs via our PE loader (`docs/voice.md`); the headset/speaker plugins target older headsets, not the G522 |
| `driver_audio_sampler_apo` | "Audio Sampler": taps playback for the audio visualizer | PipeWire monitor capture (`crates/lighting/src/audio.rs`) |
| `driver_audio_g522` | INF only: attaches the Audio Sampler to the G522 microphone (mic level, mic test) | PipeWire source monitoring; G522 voice processing is on the headset (Logivoice, HID++ `0x0900`–`0x0908`) |
| `driver_hid_virtual` | Virtual HID device for key/mouse emulation (macros, remaps) | `/dev/uinput` (needs a udev rule) |
| `driver_logi_lamparray` | Windows Dynamic Lighting (LampArray) | n/a |
| `driver_usb`, `driver_audio_devices` | USB and audio device plumbing | kernel drivers (`hid-logitech-dj`, `snd-usb-audio`) |

## Device pictures

Imported at runtime with `lhub import-ghub <G HUB depots dir>` into `~/.local/share/lhub/devices/`.
G515, G522 and Yeti GX pictures and LED geometry work (live viewer overlay). The G502 X PLUS
device pictures in its package are encrypted (G HUB fetches the key online); they are skipped,
and the mouse shows its unencrypted product card plus a zone strip.

## App settings

| G HUB setting | Status | Notes |
|---|---|---|
| Start on login | built | Settings page toggles the systemd user unit (writes one for dev builds) |
| Notifications: low battery | built | Desktop notification at 15 %, once per discharge; general and Caps/Num lock still planned |
| Notifications: general, Caps/Num lock | planned | `notifications_enabled`, `low_battery_notifications_enabled`, `lock_notifications_enabled` |
| Language | planned | UI is English only |
| Home layout (grid/list) | planned | `home_layout` |
| Tray icon with device list, DPI and battery | planned | `/system_tray/*` |
| Analytics, accounts, device recommendations | n/a | |
| G HUB data folders: scan and import | built | Settings page: pictures, LED layouts, VO!CE engine and presets (reads settings.db) |
| Add a device (starter definition) | built | Settings page writes `~/.local/share/lhub/drivers/<device>.toml` |
| VO!CE preset test (record, compare presets on one take) | built | `lhub-voice --process` runs presets offline on the recording |

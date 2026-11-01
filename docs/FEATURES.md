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
| Device pictures | planned | Imported at runtime from the user's G HUB install; not in this install for G515/G502 X/G522/Yeti GX |
| Firmware update (DFU) | n/a | Excluded on purpose |
| Receiver pairing / unpairing | planned | `/devices/pairable`, `0x1894` |

## Lighting

| G HUB feature | Status | Notes |
|---|---|---|
| Fixed / breathing / color cycle (device effects) | done | Effect index looked up by id per device |
| Brightness | done | Scaled to each device's maximum; applied to live effects too |
| Per-key painting (freestyle) | built | Library + CLI; no UI painter yet |
| Animated host effects | done | Wave, starlight, pulse (live), visualizer |
| Firmware reactive ripple (G515 `0x0017`) | done | Verified by user |
| Other firmware all-zones effects (G515 `0x000c 0x0013 0x0016 0x0018 0x0019`, G502 `0x0004 0x000c 0x000f 0x0010 0x0013`) | next | Parameters partly seen in captures |
| Audio visualizer | built | PipeWire monitor of default output |
| Screen sampler | planned | Wayland: needs xdg-desktop-portal screencast |
| LIGHTSYNC: same effect on all devices | next | |
| Palettes (vibrant, pastel, mono, signature, custom) | planned | |
| Custom effect editor, save/import/export | planned | |
| Turn off lighting after inactivity | next | RGB `0x8071` fn 7 power settings seen in captures |
| Mouse zone layout | next | |
| LampArray (Windows Dynamic Lighting) | n/a | Windows-only |

## Mouse (G502 X PLUS)

| G HUB feature | Status | Notes |
|---|---|---|
| DPI levels, default, shift | built | `0x2201` levels + active level in service and UI; shift not yet |
| Report rate | done | `0x8060` read/write verified |
| DPI cycle up/down via mouse buttons | planned | Host mode needs button spy `0x8110` |
| Hi-res wheel / wheel mode | next | `0x2121` |
| Button assignments | planned | Onboard profiles `0x8100` |
| Onboard memory mode | planned | Read back before any write |

## Keyboard (G515 LS TKL)

| G HUB feature | Status | Notes |
|---|---|---|
| Key assignments, G-shift | planned | `0x1b05` full key customization |
| Game mode (disable Win key etc.) | next | `0x4523` |
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
| Headset lighting | next | `0x0620`–`0x0622` |

## Microphone (Yeti GX)

| G HUB feature | Status | Notes |
|---|---|---|
| Gain, mute | done / built | Mute untested |
| Lighting | done | |
| Blue VO!CE 2 engine | built | Real DTS ClearVoice 2 DLL via our PE loader, offline test only |
| VO!CE presets → engine parameters | next | |
| VO!CE as a PipeWire virtual mic | next | |
| Smart lock | planned | G HUB byte `0xae` vs ours `0xca` unresolved |

## Profiles, assignments, macros

| G HUB feature | Status | Notes |
|---|---|---|
| Profiles per application, auto switching | planned | Active-window tracking on KDE Wayland via KWin |
| Persistent features (settings shared across profiles) | planned | |
| Macros (record, edit, assign) | planned | Host macros need `/dev/uinput` access |
| Lua scripting | planned | |
| Import/export profiles | planned | |

## Not applicable

Analog keyboard features (actuation point, rapid trigger, key priority), wheels and
force feedback, Trueforce, haptics, LampArray, UVC cameras, Overwolf/Discord/OBS applets,
G HUB accounts/community/marketplace, game library (`gl`), Windows audio mixing (`mixline`).

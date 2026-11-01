# Status

Last updated 2026-11-01. Feature-by-feature port status lives in `FEATURES.md`.

## Running it

```sh
target/release/lhub-app          # UI; starts the service if it isn't running
target/release/lhubd             # service on its own (log: ~/.local/state/lhub/lhubd.log)
target/release/lhub list         # CLI: devices and features (LHUB_DEBUG=1 prints every frame)
```

Device access comes from `packaging/udev/70-lhub.rules` (installed on this machine).
`packaging/arch/PKGBUILD` installs the app, the CLI, the service, the rule and a systemd user
unit (`systemctl --user enable --now lhubd`); the package itself hasn't been test-built.

## Layout

| Path | What |
|---|---|
| `crates/hidpp` | HID++ 2.0 over hidraw, centurion headset transport, decoded device features |
| `crates/lighting` | Key layout, per-key streaming engine, host effects, audio analyzer, key input |
| `crates/daemon` | `lhubd`: device workers, persistent config, socket API |
| `crates/cli` | `lhub` command-line tool and experiments |
| `crates/voice/loader` | PE loader running Logitech's Blue VO!CE 2 (DTS ClearVoice 2) DLL natively |
| `tools/voice-probes` | Offline VO!CE engine experiments |
| `app` | Tauri + Svelte UI, a client of `lhubd` |
| `packaging` | udev rule, systemd unit, Arch package |

## Verified on your hardware

- Discovery of all four devices; G522 through its dongle's bridge (when the headset is on).
- Lighting: fixed/breathing/cycle on mic, keyboard and mouse; keyboard per-key frames;
  host effects (wave ~10 fps full-board, pulse ~24 fps); keyboard firmware ripple.
- Brightness on mic (0–255) and keyboard (0–100).
- Mouse DPI and report rate (read; write-back of current values).
- Service: saves settings, applies them on connect, single instance, survives the UI closing.

## Built but not verified

- Audio visualizer effect (capture stream verified; no audio was playing to see bars).
- Mic mute; mouse live effects (zones laid out in a line until the layout is mapped).
- Sensitivity UI (backend verified via the socket, UI not opened).

## Known issues

- An app build from before the zombie-reaping fix leaves a `<defunct>` lhubd entry until
  that app window closes. Harmless.
- The G522 drops out of the device list while the headset is off; it should appear as
  "off" instead.
- DPI cycling with the mouse buttons in host mode needs the button-spy feature (`0x8110`).

## Device pictures

G HUB's install here has 309 product pictures (older G502/Yeti models among them), but none
for the G515, G502 X PLUS, G522 or Yeti GX: G HUB downloads those on demand. They are
never copied into this repo; an import step will read them from the user's own G HUB data.

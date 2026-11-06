# Status

Last updated 2026-11-06. Feature-by-feature port status lives in `FEATURES.md`.

## Running it

```sh
target/release/glowden          # UI; starts the service if it isn't running (and restarts it if it dies)
target/release/glowdend             # service on its own (log: ~/.local/state/glowden/glowdend.log)
target/release/glowdenctl list         # CLI: devices and features (GLOWDEN_DEBUG=1 prints every frame)
```

Device access comes from `packaging/udev/70-glowden.rules` (installed on this machine).
`packaging/arch/PKGBUILD` (built with `makepkg`) and `packaging/deb/build.sh` (Ubuntu 22.04
container → `dist/glowden_<version>_amd64.deb`) install the app, the CLI, the service, the
helpers, the rule, a systemd user unit and a desktop entry. Fresh installs of both
packages were tested on 2026-11-06 in clean Ubuntu 22.04 and Arch containers with a virtual
display: the app opens and starts the service, the built-in source and a definition install
work, and the public Blue VO!CE plugin matches its checksum and answers. That test found the
Arch package missing libayatana-appindicator, which the app needs to start.

## Layout

| Path | What |
|---|---|
| `crates/hidpp` | HID++ 2.0 over hidraw, centurion headset transport, decoded device features |
| `crates/lighting` | Key layout, per-key streaming engine, host effects, audio analyzer, key input |
| `crates/daemon` | `glowdend`: device workers, persistent config, socket API |
| `crates/cli` | `glowdenctl` command-line tool and experiments |
| `crates/sampler` | `glowden-sampler`: ScreenCast portal + PipeWire video → color grid for the screen sampler effect |
| `app` | Tauri + Svelte UI, a client of `glowdend` |
| `packaging` | udev rule, systemd unit, Arch package |

## Verified on your hardware

- Discovery of all four devices; G522 through its dongle's bridge (when the headset is on).
- Lighting: fixed/breathing/cycle on mic, keyboard and mouse; keyboard per-key frames;
  host effects (wave ~10 fps full-board, pulse ~24 fps); keyboard firmware ripple.
- Brightness on mic (0–255) and keyboard (0–100).
- Mouse DPI and report rate (read; write-back of current values).
- Service: saves settings, applies them on connect, single instance, survives the UI closing.
- G522 switched off behind its dongle: listed as off, at service start and when switched off
  while running.
- Package upgrade (0.2.0 → 0.2.2): the post-upgrade hook restarted the running glowdend.

## Built but not verified

- Audio visualizer effect (capture stream verified; no audio was playing to see bars).
- Mic mute; mouse live effects and the order of its 8 zones on the photo.
- Mouse DPI up/down/shift buttons (button spy `0x8110`).
- Sensitivity UI (backend verified via the socket, UI not opened).

## Known issues

- An app build from before the zombie-reaping fix leaves a `<defunct>` glowdend entry until
  that app window closes. Harmless.

## Device pictures

Keyboard, mic and headset pictures are read in place from the user's G HUB folders
(`assets.rs`); the G502 X PLUS ones are encrypted, so its definition points at Logitech's
public product photo, downloaded on first use into `~/.cache/glowden/photos`. Neither is
copied into this repo.

## Device sources and plugins

Definitions and plugins live in the glowden-devices repo and reach GlowDen as a device
source (`sources.rs`): `index.json` plus flat files, from a URL or a local folder. The
built-in source URL (`sources::DEFAULT_SOURCE`) points at
github.com/larryGF/glowden-devices (public; v0.2.0 is the latest release). This machine
also has a local build of glowden-devices (`dist/`) added as a second source. Blue
VO!CE runs in that repo's `blue-voice` plugin (JSON lines over stdin/stdout); verified here
end to end (virtual mic up, levels flowing, pictures and layouts read in place).

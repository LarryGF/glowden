# GlowDen

An open device hub for Logitech G peripherals on Linux: lighting, buttons, DPI,
microphone and headset settings, kept applied by a background service, with a desktop app.
No Wine.

GlowDen is a framework. It ships its own code and device definitions (protocol facts); you
bring the assets. Pictures, sound clips, VO!CE presets and the optional Blue VO!CE engine
are imported at runtime from your own G HUB copy (`glowdenctl import-ghub`), and nothing from
Logitech or DTS is part of this repo or its packages. GlowDen is not affiliated with Logitech;
see [`NOTICE.md`](NOTICE.md).

## Supported hardware

| Device | Working today |
|---|---|
| Yeti GX microphone | Lighting, brightness, hardware gain, mute |
| G515 LS TKL keyboard | Lighting (device effects, per-key, firmware ripple), live effects, brightness |
| G502 X PLUS mouse | Lighting, live effects, DPI levels, report rate |
| G522 LIGHTSPEED headset | Detection through its dongle; settings in progress |

Full G HUB feature inventory and port status: [`docs/FEATURES.md`](docs/FEATURES.md).
Current state and known issues: [`docs/STATUS.md`](docs/STATUS.md).

## Build

Requires the Rust toolchain (pinned by `rust-toolchain.toml`), Node with pnpm, and the
WebKitGTK 4.1, GTK 3, PipeWire, libsystemd and D-Bus development packages.

```sh
cargo build --release -p glowdend -p glowden-cli -p glowden-voice -p glowden-sampler
cd app && pnpm install && pnpm tauri build   # desktop app (after the cargo build)
```

Binaries land in `target/release/`: `glowden` (app), `glowdend`, `glowdenctl`, `glowden-voice` (VO!CE host),
`glowden-sampler` (screen sampler).

## Install

Device access needs the udev rule (hidraw for Logitech devices, key events from Logitech
keyboards for reactive effects, granted to the logged-in user):

```sh
sudo install -Dm644 packaging/udev/70-glowden.rules /usr/lib/udev/rules.d/70-glowden.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --subsystem-match=input --action=change
```

Packages install all of that (binaries, rule, systemd user unit, desktop entry):

- **Arch**: `makepkg -si` in `packaging/arch`.
- **Debian 12+ / Ubuntu 22.04+**: `packaging/deb/build.sh` builds `dist/glowden_<version>_amd64.deb`
  in an Ubuntu 22.04 container (needs Docker); install it with `sudo apt install ./dist/glowden_*.deb`.

Blue VO!CE, the voice effects and the audio visualizer need PipeWire as the sound server
(the default on Ubuntu 22.10+, Debian 12 desktops and Arch). Ubuntu 22.04 uses PulseAudio
by default, so those features need `pipewire-pulse` there.

Then start the service for your user: `systemctl --user enable --now glowdend` (the app also
starts it when it isn't running).

## Use

- `glowden` opens the desktop app and starts the service if it isn't running.
- `glowdend` keeps every device's settings in `~/.config/glowden/config.json` and re-applies them
  when a device connects or wakes. Live effects (wave, pulse, starlight, visualizer) are
  drawn by the service and keep running without the app.
- `glowdenctl` is a CLI for inspection and experiments: `glowdenctl list`, `glowdenctl <device> effects`,
  `GLOWDEN_DEBUG=1 glowdenctl …` prints every HID++ frame.

### Device pictures

G HUB downloads per-device resource packages (pictures, LED geometry). To show them, point
the importer at a G HUB `depots` folder you have access to (for example copied from
`C:\ProgramData\LGHUB\depots`):

```sh
glowdenctl import-ghub /path/to/LGHUB/depots
```

Files go to `~/.local/share/glowden/devices/<model>/` and stay on your machine. Encrypted
assets are skipped.

### Blue VO!CE

See [`docs/voice.md`](docs/voice.md). The engine DLL comes from your own G HUB install.

## Layout

| Path | What |
|---|---|
| `crates/hidpp` | HID++ 2.0 over hidraw, headset (centurion) transport, device features |
| `crates/lighting` | Key layout, per-key streaming, effects, audio analyzer, key input |
| `crates/daemon` | `glowdend`: per-device workers, persistent config, Unix socket API |
| `crates/cli` | `glowdenctl` CLI |
| `crates/voice/loader` | PE loader that runs the Blue VO!CE 2 DLL natively |
| `app` | Tauri + Svelte desktop app (client of `glowdend`) |
| `packaging` | udev rule, systemd unit, Arch package |
| `tools/voice-probes` | Offline experiments against the VO!CE engine |
| `docs` | Feature inventory, status, protocol notes |

## License

GlowDen is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. Files you import from G HUB stay under Logitech's and DTS's terms; see
[`NOTICE.md`](NOTICE.md).

# lhub

Native Linux control for Logitech G peripherals: lighting, sensitivity, microphone and
headset settings, kept applied by a background service, with a desktop app in the spirit of
G HUB. No Wine, no Windows components at runtime except the optional Blue VO!CE engine,
which runs from your own G HUB copy through a small native loader.

Protocols were worked out from USB captures of G HUB, its agent and UI bundles, and the
devices' own feature lists. No Logitech code, pictures or presets are part of this repo.

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
WebKitGTK 4.1, GTK 3 and PipeWire development packages.

```sh
cargo build --release -p lhubd -p lhub-cli   # service and CLI
cd app && pnpm install && pnpm tauri build   # desktop app
```

Binaries land in `target/release/`: `lhub-app`, `lhubd`, `lhub`.

## Install

Device access needs the udev rule (hidraw for Logitech devices, key events from Logitech
keyboards for reactive effects, granted to the logged-in user):

```sh
sudo install -Dm644 packaging/udev/70-lhub.rules /usr/lib/udev/rules.d/70-lhub.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --subsystem-match=input --action=change
```

On Arch, `packaging/arch/PKGBUILD` installs the binaries, the rule and a systemd user unit:
`makepkg -si` in `packaging/arch`, then `systemctl --user enable --now lhubd`.

## Use

- `lhub-app` opens the desktop app and starts the service if it isn't running.
- `lhubd` keeps every device's settings in `~/.config/lhub/config.json` and re-applies them
  when a device connects or wakes. Live effects (wave, pulse, starlight, visualizer) are
  drawn by the service and keep running without the app.
- `lhub` is a CLI for inspection and experiments: `lhub list`, `lhub <device> effects`,
  `LHUB_DEBUG=1 lhub …` prints every HID++ frame.

### Device pictures

G HUB downloads per-device resource packages (pictures, LED geometry). To show them, point
the importer at a G HUB `depots` folder you have access to (for example copied from
`C:\ProgramData\LGHUB\depots`):

```sh
lhub import-ghub /path/to/LGHUB/depots
```

Files go to `~/.local/share/lhub/devices/<model>/` and stay on your machine. Encrypted
assets are skipped.

### Blue VO!CE

See [`docs/voice.md`](docs/voice.md). The engine DLL comes from your own G HUB install.

## Layout

| Path | What |
|---|---|
| `crates/hidpp` | HID++ 2.0 over hidraw, headset (centurion) transport, device features |
| `crates/lighting` | Key layout, per-key streaming, effects, audio analyzer, key input |
| `crates/daemon` | `lhubd`: per-device workers, persistent config, Unix socket API |
| `crates/cli` | `lhub` CLI |
| `crates/voice/loader` | PE loader that runs the Blue VO!CE 2 DLL natively |
| `app` | Tauri + Svelte desktop app (client of `lhubd`) |
| `packaging` | udev rule, systemd unit, Arch package |
| `tools/voice-probes` | Offline experiments against the VO!CE engine |
| `docs` | Feature inventory, status, protocol notes |

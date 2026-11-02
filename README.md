# GlowDen

An open device hub for Logitech G peripherals on Linux: lighting, buttons, DPI,
microphone and headset settings, kept applied by a background service, with a desktop app.
No Wine.

GlowDen is a framework. It ships its own code and device definitions (protocol facts); you
bring the assets. Pictures, sound clips, VO!CE presets and the optional Blue VO!CE engine
are read at runtime from G HUB folders you choose (Settings → G HUB files), and nothing from
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
cargo build --release -p glowdend -p glowden-cli -p glowden-sampler
cd app && pnpm install && pnpm tauri build   # desktop app (after the cargo build)
```

Binaries land in `target/release/`: `glowden` (app), `glowdend`, `glowdenctl` and
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

### Devices and plugins

What GlowDen knows about each device (kind, key layout, LED regions, buttons, firmware
effects) comes from device sources: GlowDen's own
([glowden-devices](https://github.com/larryGF/glowden-devices)) is always listed, and you can
add others in Settings → Device sources. A device that connects without a definition is
looked up there automatically, and you can install or remove definitions one by one.

Some devices need code as well, shipped by the source as a plugin (Blue VO!CE on the Yeti
GX is one). Plugins are programs, so they run only after you allow that source's plugins.

### G HUB files

Device pictures, LED geometry, sound clips, the Blue VO!CE presets and its engine come from
a G HUB installation you have, for example on a dual-boot Windows drive: mount it and add
the drive (or any folder with the files) in Settings → G HUB files. GlowDen reads the files
where they are and never copies them; what you point it at, and under which terms you got
it, is your responsibility. Encrypted files are skipped.

## Layout

| Path | What |
|---|---|
| `crates/hidpp` | HID++ 2.0 over hidraw, headset (centurion) transport, device features |
| `crates/lighting` | Key layout, per-key streaming, effects, audio analyzer, key input |
| `crates/daemon` | `glowdend`: per-device workers, persistent config, Unix socket API |
| `crates/cli` | `glowdenctl` CLI |
| `app` | Tauri + Svelte desktop app (client of `glowdend`) |
| `packaging` | udev rule, systemd unit, Arch package |
| `docs` | Feature inventory, status, protocol notes |

## License

GlowDen is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. Files you import from G HUB stay under Logitech's and DTS's terms; see
[`NOTICE.md`](NOTICE.md).

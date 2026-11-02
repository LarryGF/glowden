# Contributing to GlowDen

Thanks for helping. This repository is the engine: the service (`glowdend`), the desktop
app, the HID++ protocol code and packaging. Device definitions and device-specific plugins
live in [glowden-devices](https://github.com/larryGF/glowden-devices); if you want to add or
fix support for one device, start there.

## Ground rules

- **No Logitech or DTS material.** Don't commit their binaries, pictures, sounds, presets,
  translations or other files, and don't paste text from their apps. GlowDen reads such
  files at runtime from folders the user chooses, in place.
- **No decryption** of protected files, and no code that works around protection.
- **Interoperability facts only.** Protocol knowledge comes from the devices themselves and
  from observing USB traffic; describe what bytes do, in your own words.
- **Nothing device-specific here.** Behaviour shared by every device with an HID++ feature
  belongs in `crates/hidpp`; anything one device needs belongs in a definition or a plugin
  in glowden-devices.

## Development setup

You need the Rust toolchain, Node 20+ with pnpm, and the WebKitGTK 4.1, GTK 3, PipeWire,
libsystemd and D-Bus development packages, plus the udev rule from `packaging/udev`
(see the README).

```sh
cargo build --release -p glowdend -p glowden-cli -p glowden-sampler
cd app && pnpm install && pnpm tauri build
```

Build the app with `pnpm tauri build`: a plain `cargo build` of the workspace produces an
app binary that expects the dev server and shows "Connection refused". Run the service with
`target/release/glowdend` (only one runs at a time; the app starts it if needed).

To work on definitions at the same time, build a local source from a glowden-devices
checkout (`tools/make-index.py dist`) and add its `dist` folder in Settings → Device sources.

## Testing on hardware

- `glowdenctl list` and `GLOWDEN_DEBUG=1` frame traces are the main tools. Stop the service
  before CLI experiments that talk to the same device.
- Never send firmware/DFU commands, and never write onboard profile memory (`0x8100`)
  without reading it back first.
- Prefer reads. Test a write by writing back the value the device already has.

## Code style

- Rust 2024 edition; run `cargo fmt` before committing. Comments are for non-obvious protocol
  facts and decisions, not for restating the code.
- UI: Svelte 5 runes, colors as CSS variables in `app/src/app.css`.
- New dependencies only with a clear need.

## Commits and releases

Commits follow [Conventional Commits](https://www.conventionalcommits.org): `feat: …`,
`fix: …`, `docs: …`, `ci: …`, `chore: …` (`feat!:` or a `BREAKING CHANGE:` footer for
breaking changes). release-please turns them into release PRs and changelogs; merging a
release PR publishes the .deb and Arch packages to the GitHub release.

## License

By contributing you agree that your contributions are licensed under MIT OR Apache-2.0, like
the rest of the project.

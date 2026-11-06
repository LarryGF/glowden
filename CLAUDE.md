# CLAUDE.md

Guidance for working in this repo. Read `docs/STATUS.md` and `docs/FEATURES.md` first; they
track what is done, verified, and next.

## Architecture

- `glowdend` (crates/daemon) is the only process that should talk to devices. One worker
  thread per device owns its `hidpp::Device`; the UI and CLI-over-socket send JSON lines to
  `$XDG_RUNTIME_DIR/glowden.sock`. Config: `~/.config/glowden/config.json`, keyed by stable id
  `<usb port>:<product id>`.
- The app (`app/`) never opens devices; `app/src-tauri/src/main.rs` only relays requests.
- `crates/hidpp` holds protocol code. Decoded device behaviour goes in `devices.rs` with a
  comment saying where the bytes came from (capture, device reply, Solaar notes).
- `crates/lighting` renders host effects; `Streamer::frame` sends only changed zones.

## Device safety rules

- Never send firmware/DFU commands.
- Never write onboard profile memory (`0x8100` writes) before reading the current contents
  back and confirming the format.
- Test writes by writing back the value the device already has first.
- Prefer reads; when a write is needed on the user's hardware, say what will visibly change.

## Building

- Build the app with `cd app && pnpm tauri build`. A plain `cargo build` also rebuilds
  `target/release/glowden`, but without the `custom-protocol` feature, so the window loads
  the dev server (`localhost:1420`) and shows "Connection refused". After a workspace
  `cargo build`, run `pnpm tauri build` again.

## Releasing and installing

- release-please opens a release PR after Conventional Commits land on `main`; merging it
  builds the Arch and .deb packages (`gh run watch` the release-please run).
- Once the release run has passed, install the new package on this machine yourself; don't
  hand the command to the user. `sudo` needs a password Claude can't type, so use `pkexec`,
  which asks the user to approve in a desktop dialog:
  ```sh
  cd ~/git/logitech/scratch
  gh release download vX.Y.Z -R LarryGF/glowden -p '*.pkg.tar.zst' --clobber
  # pkexec doesn't keep the working directory: pass an absolute path
  pkexec pacman -U --noconfirm "$PWD/glowden-X.Y.Z-1-x86_64.pkg.tar.zst"
  ```
- Then check `pacman -Q glowden`. The package's upgrade hook restarts `glowdend`. The app
  isn't restarted: ask the user to reopen GlowDen, or restart it only with their OK.

## Testing against hardware

- `target/release/glowdenctl list` and `GLOWDEN_DEBUG=1` frame traces are the main tools.
- Stop the service before CLI experiments that stream to the same device:
  `for p in $(pgrep -x glowdend); do kill $p; done` — never `pkill -f glowdend` (it matches the
  calling shell). Restart with `setsid -f target/release/glowdend >> ~/.local/state/glowden/glowdend.log 2>&1`.
- Wireless devices sleep; the transport retries with growing timeouts. Discovery backs off
  silent nodes for 30 s.
- hidraw numbers change across replug; use the stable ids.

## Logitech material

- Do not commit Logitech binaries, pictures, presets or data files. The one exception is a
  README screenshot of the app showing device pictures as GlowDen renders them from the
  user's G HUB files (`docs/screenshots/home-ghub.png`); keep those few and never commit a
  picture on its own. Pictures, LED geometry,
  sounds, presets and the VO!CE DLL are read in place from folders the user chooses
  (`crates/daemon/src/assets.rs`); never copy them.
- Device definitions and plugins live in the separate glowden-devices repo (device
  sources, `crates/daemon/src/sources.rs`); nothing device-specific goes in this repo.
- Do not decrypt protected assets.
- Original effects and presets only: generic effect types are fine, Logitech's named
  keyframe animations are not to be transcribed.

## Conventions

- Rust edition 2024, `cargo fmt` before committing, no new dependencies without need.
- Comments sparingly, for non-obvious protocol facts.
- Commits without co-author or generated-by trailers.
- UI: Svelte 5 runes, colors as CSS variables in `app/src/app.css`.

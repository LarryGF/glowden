# CLAUDE.md

Guidance for working in this repo. Read `docs/STATUS.md` and `docs/FEATURES.md` first; they
track what is done, verified, and next.

## Architecture

- `lhubd` (crates/daemon) is the only process that should talk to devices. One worker
  thread per device owns its `hidpp::Device`; the UI and CLI-over-socket send JSON lines to
  `$XDG_RUNTIME_DIR/lhub.sock`. Config: `~/.config/lhub/config.json`, keyed by stable id
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

## Testing against hardware

- `target/release/lhub list` and `LHUB_DEBUG=1` frame traces are the main tools.
- Stop the service before CLI experiments that stream to the same device:
  `for p in $(pgrep -x lhubd); do kill $p; done` — never `pkill -f lhubd` (it matches the
  calling shell). Restart with `setsid -f target/release/lhubd >> ~/.local/state/lhub/lhubd.log 2>&1`.
- Wireless devices sleep; the transport retries with growing timeouts. Discovery backs off
  silent nodes for 30 s.
- hidraw numbers change across replug; use the stable ids.

## Logitech material

- Do not commit Logitech binaries, pictures, presets or data files. Pictures and LED
  geometry are imported at runtime (`lhub import-ghub`); the VO!CE DLL is the user's own copy.
- Do not decrypt protected assets.
- Original effects and presets only: generic effect types are fine, Logitech's named
  keyframe animations are not to be transcribed.

## Conventions

- Rust edition 2024, `cargo fmt` before committing, no new dependencies without need.
- Comments sparingly, for non-obvious protocol facts.
- Commits without co-author or generated-by trailers.
- UI: Svelte 5 runes, colors as CSS variables in `app/src/app.css`.

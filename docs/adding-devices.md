# Adding a device

Most Logitech G devices speak HID++ 2.0 and report their features. lhub drives the standard
ones for any device that has them, without a definition:

| Feature | What works automatically |
|---|---|
| `0x8071` RGB effects | Fixed, breathing, color cycle (effects looked up by id) |
| `0x8081` per-key lighting | Live effects (zones laid out in a line without a layout) |
| `0x8040` brightness | Brightness, scaled to the device's maximum |
| `0x2201` adjustable DPI, `0x8060` report rate | Sensitivity panel |
| `0x1004` unified battery | Battery level |
| `0x8370` microphone | Gain, mute |

Run `lhub list` to see what your device reports.

## Definition files

Anything that isn't discoverable goes in a TOML file. lhub reads, in order (later wins for
the same product id):

1. `/usr/share/lhub/drivers/` (shipped definitions)
2. `drivers/` in the repo (development builds)
3. `~/.local/share/lhub/drivers/` (your own)
4. directories in `$LHUB_DRIVERS`

Restart the service after adding a file: `systemctl --user restart lhubd`.

```toml
name = "G515 LS TKL"
kind = "keyboard"            # keyboard | mouse | headset | microphone
products = [0x40b4]          # USB product ids (receiver-attached devices: the device's own id)
ghub_model = "g515_ls_tkl"   # folder name used by `lhub import-ghub` for pictures/geometry

[layout]
keys = "iso-tkl"             # built-in key geometry for live effects
zones = "hid-usage"          # zone id = USB key code − 3 (otherwise zone ids are used as-is)

[input]
reactive = true              # read key presses from the device for reactive effects

# Effects stored in the firmware. Find candidates with `lhub <device> effects`
# (cluster 0xff lists the all-zones effects) and try them with
# `lhub <device> raw-effect <cluster> <index> <hex params>`.
[[firmware_effect]]
id = "ripple"
label = "Ripple"
cluster = 0xff
effect_id = 0x0017
params = "{r}{g}{b}ff01000014"   # up to 10 bytes; {r}{g}{b} = chosen color
```

## Product photos

When G HUB has no usable picture for a device, a definition can point at the manufacturer's
public product photo instead. lhubd downloads it once into
`~/.local/share/lhub/devices/<model>/product-photo.png`; it is never shipped with lhub.

```toml
[layout.photo]
url = "https://…/top-view.png"
crop = [x, y, width, height]   # the device inside the picture
cutout = "vivid"               # lit areas in the photo become openings for live colors

[[layout.photo.led]]
zone = 1
path = "M0 850 L517 850 …"     # SVG path in cropped-photo pixels
```

The LED regions only need to cover their part of the lit area; with `cutout = "vivid"` the
strongly colored pixels inside them decide what lights up. See `drivers/g502-x-plus.toml`.

## When data isn't enough

New protocol behaviour (a feature lhub doesn't implement yet) goes in `crates/hidpp/src/devices.rs`
as a function per feature, keyed by HID++ feature id, so every device with that feature gets
it. Record where the bytes came from (USB capture, device reply) in a comment. The capture
workflow (Windows VM + usbmon) is described in `docs/STATUS.md`.

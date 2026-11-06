# Notice

GlowDen is an independent, community-made framework for configuring gaming peripherals on
Linux. It is not affiliated with, endorsed by or supported by Logitech, DTS or Xperi.

## Trademarks

Logitech, Logitech G, G HUB, LIGHTSYNC, LIGHTSPEED, Blue VO!CE and the product names of
Logitech devices are trademarks of Logitech International S.A. DTS and ClearVoice are
trademarks of DTS, Inc. (Xperi). They are used here only to say which devices and files
GlowDen works with.

## What GlowDen contains and what it doesn't

GlowDen is its own code under the MIT or Apache-2.0 license (`LICENSE-MIT`, `LICENSE-APACHE`).
It talks to devices over their USB and HID++ interfaces, which were worked out from the
devices themselves and from captures of their traffic, for interoperability.

It ships no Logitech or DTS software, firmware, pictures, sounds or presets, and no device
definitions: those come from device sources (GlowDen's own is glowden-devices), and
describe protocol facts such as product ids, feature numbers and parameter layouts.

Some definitions point at a product photo Logitech publishes on its website; GlowDen
downloads it on the user's machine on first use (`~/.cache/glowden/photos`) and doesn't
redistribute it.

Users bring their own assets. GlowDen reads files from folders the user chooses, such as
the user's own G HUB installation on a dual-boot drive: device pictures, sound clips, VO!CE
presets and the Blue VO!CE audio engine. It reads them in place and never copies them.
Those files stay under their owners' licenses; whether you may use them this way depends on
the terms you accepted for that software, and what you point GlowDen at is your
responsibility. GlowDen never decrypts protected files.

## Third-party components

Binaries built from this repository include Rust crates and npm packages under their own
licenses: mostly MIT and/or Apache-2.0, plus BSD, ISC, Zlib and Unicode-3.0. `cssparser`,
`cssparser-macros`, `dtoa-short`, `option-ext` and `selectors` are under MPL-2.0 and are
used unmodified; their source is on crates.io. GlowDen links dynamically to system libraries
(WebKitGTK, GTK, PipeWire, libsystemd, libdbus) under their own licenses.

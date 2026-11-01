//! HID++ 2.0 feature ids we use or recognise.

pub const ROOT: u16 = 0x0000;
pub const FEATURE_SET: u16 = 0x0001;
pub const FIRMWARE: u16 = 0x0003;
pub const DEVICE_NAME: u16 = 0x0005;
pub const FRIENDLY_NAME: u16 = 0x0007;
pub const UNIFIED_BATTERY: u16 = 0x1004;
pub const WIRELESS_STATUS: u16 = 0x1d4b;
pub const HIRES_WHEEL: u16 = 0x2121;
pub const ADJUSTABLE_DPI: u16 = 0x2201;
pub const BRIGHTNESS: u16 = 0x8040;
pub const RGB_EFFECTS: u16 = 0x8071;
pub const PER_KEY_LIGHTING_V2: u16 = 0x8081;
pub const ONBOARD_PROFILES: u16 = 0x8100;
pub const PROFILE_MANAGEMENT: u16 = 0x8101;
pub const MICROPHONE: u16 = 0x8370;

/// Feature names as G HUB's agent labels its handlers. Headset (centurion) devices use the
/// low ranges with their own meanings, so they are looked up separately.
pub fn centurion_name(id: u16) -> Option<&'static str> {
    Some(match id {
        0x0001 => "feature set",
        0x0002 => "engineering test",
        0x0004 => "profile",
        0x0005 => "multi-host control",
        0x0100 => "device info",
        0x0104 => "battery state of charge",
        0x0108 => "auto sleep",
        0x010a => "DFU",
        0x020d => "advanced parametric EQ",
        0x020e => "mic test",
        0x0305 => "Bluetooth host info",
        0x0601 => "mic mute",
        0x0604 => "sidetone",
        0x060b => "headset tones",
        0x0611 => "mic gain",
        0x0617 => "usage tracking",
        0x0618 => "battery saver",
        0x0620 => "RGB host mode",
        0x0621 => "RGB onboard effects",
        0x0622 => "RGB signature effects",
        0x0623 => "?",
        0x0900 => "Logivoice",
        0x0901 => "Logivoice noise reduction",
        0x0902 => "Logivoice noise gate",
        0x0903 => "Logivoice compressor",
        0x0904 => "Logivoice de-esser",
        0x0905 => "Logivoice de-popper",
        0x0906 => "Logivoice limiter",
        0x0907 => "Logivoice high-pass filter",
        0x0908 => "Logivoice parametric EQ",
        _ => return None,
    })
}

pub fn name(id: u16) -> Option<&'static str> {
    Some(match id {
        ROOT => "root",
        FEATURE_SET => "feature set",
        FIRMWARE => "firmware info",
        DEVICE_NAME => "device name",
        FRIENDLY_NAME => "friendly name",
        0x0011 => "property access",
        0x0020 => "config change",
        0x00c2 => "DFU (signed)",
        0x00d0 => "DFU",
        UNIFIED_BATTERY => "unified battery",
        0x0021 => "crypto identifier",
        0x1500 => "force pairing",
        0x1602 => "?",
        0x1800 => "generic test",
        0x1801 => "manufacturing mode",
        0x1802 => "device reset",
        0x1805 => "OOB state",
        0x1806 => "configurable device properties",
        0x1807 => "configurable properties",
        0x1814 => "change host",
        0x1815 => "hosts info",
        0x1890 => "RF test",
        0x1894 => "lightspeed pairing",
        0x18a1 => "LED state",
        0x4610 => "multi-roller",
        0x8060 => "report rate",
        0x8110 => "mouse button spy",
        0x1b05 => "full key customization",
        0x1b10 => "control list",
        WIRELESS_STATUS => "wireless status",
        0x1e00 => "enable hidden features",
        HIRES_WHEEL => "hi-res wheel",
        ADJUSTABLE_DPI => "adjustable DPI",
        0x4523 => "keyboard disable controls",
        0x4540 => "keyboard layout",
        BRIGHTNESS => "brightness",
        0x8051 => "logi modifiers",
        0x8101 => "profile management",
        RGB_EFFECTS => "RGB effects",
        PER_KEY_LIGHTING_V2 => "per-key lighting v2",
        ONBOARD_PROFILES => "onboard profiles",
        MICROPHONE => "microphone",
        _ => return None,
    })
}

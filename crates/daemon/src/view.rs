//! What the UI is told about a device: identity, kind, capabilities, features.

use hidpp::devices;
use hidpp::discover::Found;
use hidpp::features;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct FeatureView {
    pub id: String,
    pub name: String,
    pub version: u8,
}

#[derive(Serialize, Clone)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

#[derive(Serialize, Clone)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub product: String,
    pub connection: &'static str,
    pub battery: Option<Battery>,
    pub caps: Vec<&'static str>,
    pub features: Vec<FeatureView>,
}

pub fn build(f: &mut Found) -> DeviceView {
    let dev = &mut f.device;
    let centurion = dev.is_centurion();
    let feats = dev.features().unwrap_or_default();
    let has = |id: u16| feats.iter().any(|(f, _)| *f == id);

    let kind = if has(features::MICROPHONE) {
        "microphone"
    } else if centurion {
        "headset"
    } else if has(0x4540) {
        "keyboard"
    } else if has(features::ADJUSTABLE_DPI) {
        "mouse"
    } else {
        "device"
    };

    let mut caps = Vec::new();
    if has(features::RGB_EFFECTS) {
        caps.push("lighting");
    }
    if has(features::PER_KEY_LIGHTING_V2) {
        caps.push("per_key");
    }
    if kind == "keyboard" {
        caps.push("reactive");
        if devices::cluster_effects(dev, 0xff).is_ok_and(|l| l.iter().any(|(_, id)| *id == 0x0017))
        {
            caps.push("firmware_ripple");
        }
    }
    if has(features::BRIGHTNESS) {
        caps.push("brightness");
    }
    if has(features::MICROPHONE) {
        caps.extend(["mic_gain", "mic_mute", "blue_voice"]);
    }
    if centurion {
        caps.extend(["equalizer", "sidetone", "logivoice"]);
    }
    if has(features::ADJUSTABLE_DPI) {
        caps.push("dpi");
    }
    if has(0x1b05) || has(features::ONBOARD_PROFILES) {
        caps.push("assignments");
    }

    let battery = has(features::UNIFIED_BATTERY)
        .then(|| devices::battery(dev).ok())
        .flatten()
        .map(|(percent, charging)| Battery { percent, charging });

    let features = feats
        .iter()
        .map(|&(id, version)| FeatureView {
            id: format!("0x{id:04x}"),
            name: if centurion {
                features::centurion_name(id)
            } else {
                features::name(id)
            }
            .unwrap_or("unknown")
            .to_string(),
            version,
        })
        .collect();

    DeviceView {
        id: f.id.clone(),
        name: f.name.trim_start_matches("Logitech ").to_string(),
        kind,
        product: format!("{:04x}:{:04x}", dev.info.vendor, dev.info.product),
        connection: if dev.info.wireless || centurion {
            "LIGHTSPEED"
        } else {
            "USB"
        },
        battery,
        caps,
        features,
    }
}

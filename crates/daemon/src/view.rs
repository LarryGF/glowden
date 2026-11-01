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
    /// G HUB model name, used to find imported pictures and layouts.
    pub model: Option<String>,
    pub firmware_effects: Vec<FirmwareEffectView>,
    /// Zone ids are USB key codes − 3 (from the driver file).
    #[serde(skip)]
    pub hid_usage_zones: bool,
    /// Show only the first view of multi-view lighting pictures.
    #[serde(skip)]
    pub first_view: bool,
}

#[derive(Serialize, Clone)]
pub struct FirmwareEffectView {
    pub id: String,
    pub label: String,
}

pub fn build(f: &mut Found, driver: Option<&crate::drivers::Driver>) -> DeviceView {
    let dev = &mut f.device;
    let centurion = dev.is_centurion();
    let feats = dev.features().unwrap_or_default();
    let has = |id: u16| feats.iter().any(|(f, _)| *f == id);

    let guessed = if has(features::MICROPHONE) {
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
    let kind: &'static str = match driver.and_then(|d| d.kind.as_deref()) {
        Some("keyboard") => "keyboard",
        Some("mouse") => "mouse",
        Some("headset") => "headset",
        Some("microphone") => "microphone",
        _ => guessed,
    };

    let mut caps = Vec::new();
    if has(features::RGB_EFFECTS) {
        caps.push("lighting");
    }
    if has(features::PER_KEY_LIGHTING_V2) {
        caps.push("per_key");
    }
    if driver.is_some_and(|d| d.input.reactive) {
        caps.push("reactive");
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
        model: driver.and_then(|d| d.ghub_model.clone()),
        firmware_effects: driver
            .map(|d| {
                d.firmware_effects
                    .iter()
                    .map(|e| FirmwareEffectView {
                        id: e.id.clone(),
                        label: e.label.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        hid_usage_zones: driver.is_some_and(|d| d.layout.zones.as_deref() == Some("hid-usage")),
        first_view: driver.is_some_and(|d| d.layout.lighting_view.as_deref() == Some("first")),
    }
}

/// Picture imported from G HUB for `model`, as a data URL. Front views first, dark
/// variants preferred.
pub fn image(model: &str) -> Option<String> {
    let dir = std::path::PathBuf::from(std::env::var_os("HOME")?)
        .join(".local/share/lhub/devices")
        .join(model);
    // Product views only; splash screens and promo cards are marketing art, not the device.
    const PREFERRED: [&str; 6] = [
        "front.png",
        "front_black.webp",
        "front_graphite.webp",
        "images/device/front_graphite.webp",
        "device_black.webp",
        "front_white.webp",
    ];
    let path = PREFERRED.iter().map(|f| dir.join(f)).find(|p| p.exists())?;
    let bytes = std::fs::read(&path).ok()?;
    let mime = if path.extension()? == "png" {
        "image/png"
    } else {
        "image/webp"
    };
    Some(format!("data:{mime};base64,{}", base64(&bytes)))
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Pixel size of a PNG or WebP file, from its header.
fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    if bytes.starts_with(b"\x89PNG") && bytes.len() > 24 {
        return Some((be(&bytes[16..20]), be(&bytes[20..24])));
    }
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let le24 = |b: &[u8]| b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16;
    match &bytes[12..16] {
        b"VP8X" => Some((le24(&bytes[24..27]) + 1, le24(&bytes[27..30]) + 1)),
        b"VP8L" => {
            let b = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
            Some(((b & 0x3fff) + 1, ((b >> 14) & 0x3fff) + 1))
        }
        b"VP8 " => Some((
            u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3fff,
            u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3fff,
        )),
        _ => None,
    }
}

/// Device picture plus LED shapes in its pixel space, from the imported G HUB metadata.
/// Shape ids are zone ids, except on keyboards where they are HID usages.
pub fn layout(model: &str, keyboard: bool, first_view: bool) -> Option<serde_json::Value> {
    use serde_json::{Value, json};
    let dir = std::path::PathBuf::from(std::env::var_os("HOME")?)
        .join(".local/share/lhub/devices")
        .join(model);
    let meta: Value =
        serde_json::from_slice(&std::fs::read(dir.join("metadata.json")).ok()?).ok()?;
    let images = meta["images"].as_array()?;
    let has_shapes = |img: &Value| {
        img["zones"].as_array().is_some_and(|z| {
            z.iter()
                .any(|z| z["components"].as_array().is_some_and(|c| !c.is_empty()))
        })
    };
    let img = images
        .iter()
        .find(|i| i["key"] == "device_image_lighting" && has_shapes(i))
        .or_else(|| images.iter().find(|i| has_shapes(i)))?;
    let (w, h) = (
        img["origin"]["width"].as_u64()? as u32,
        img["origin"]["height"].as_u64()? as u32,
    );

    // The picture this geometry belongs to: same pixel size, lighting/front/dark variants first.
    let mut candidates = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "webp")) {
                candidates.push(p);
            }
        }
    }
    let rank = |p: &std::path::Path| {
        let n = p.to_string_lossy().to_lowercase();
        (
            !n.contains("lighting") as u8,
            n.contains("white") as u8,
            n.contains("splash") as u8,
            n.contains("promotion") as u8,
        )
    };
    candidates.sort_by_key(|p| rank(p));
    let (path, bytes) = candidates.into_iter().find_map(|p| {
        let b = std::fs::read(&p).ok()?;
        // Pictures can differ from the geometry's stated size by a pixel or two.
        let (iw, ih) = image_size(&b)?;
        (iw.abs_diff(w) <= w / 100 + 2 && ih.abs_diff(h) <= h / 100 + 2).then_some((p, b))
    })?;
    let mime = if path.extension()? == "png" {
        "image/png"
    } else {
        "image/webp"
    };

    // Multi-view lighting pictures list one bounds box per view; optionally show the first.
    let crop = if first_view {
        img["coordinate_transforms"]
            .as_array()
            .and_then(|t| t.first())
            .map(|t| t["bounds"].clone())
    } else {
        None
    };

    let mut shapes = Vec::new();
    for zone in img["zones"].as_array()? {
        for c in zone["components"].as_array().into_iter().flatten() {
            let id = c["id"].as_u64()? as u8;
            let zone_id = if keyboard {
                lighting::layout::zone_for_usage(id)
            } else {
                Some(id)
            };
            let Some(zone_id) = zone_id else { continue };
            for r in c["renderings"].as_array().into_iter().flatten() {
                shapes.push(json!({ "zone": zone_id, "shape": r }));
            }
        }
    }
    Some(json!({
        "width": w,
        "height": h,
        "image": format!("data:{mime};base64,{}", base64(&bytes)),
        "shapes": shapes,
        // Keyboards light their legends; other devices tint the LED regions.
        "legends": keyboard,
        "crop": crop,
    }))
}

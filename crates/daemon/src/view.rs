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
    /// Estimated hours left at the current settings (needs the imported power model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<f32>,
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
    #[serde(skip)]
    pub fallback_picture: Option<std::path::PathBuf>,
    #[serde(skip)]
    pub picture_clip: Option<String>,
    #[serde(skip)]
    pub photo: Option<crate::drivers::Photo>,
    /// Switched off behind a dongle that's still plugged in (a G522 headset).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

impl DeviceView {
    pub fn seen(&self) -> crate::config::Seen {
        crate::config::Seen {
            name: self.name.clone(),
            kind: self.kind.to_string(),
            product: self.product.clone(),
            model: self.model.clone(),
        }
    }

    /// A device met before that doesn't answer now.
    pub fn off(id: &str, seen: &crate::config::Seen) -> DeviceView {
        DeviceView {
            id: id.to_string(),
            name: seen.name.clone(),
            kind: match seen.kind.as_str() {
                "keyboard" => "keyboard",
                "mouse" => "mouse",
                "headset" => "headset",
                "microphone" => "microphone",
                _ => "device",
            },
            product: seen.product.clone(),
            connection: "LIGHTSPEED",
            battery: None,
            caps: Vec::new(),
            features: Vec::new(),
            model: seen.model.clone(),
            firmware_effects: Vec::new(),
            hid_usage_zones: false,
            first_view: false,
            fallback_picture: None,
            picture_clip: None,
            photo: None,
            off: true,
        }
    }
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
    if has(0x8110) {
        // Mouse buttons can be assigned on the host through the button spy.
        caps.push("buttons");
    }

    let battery = (has(features::UNIFIED_BATTERY) || centurion && has(devices::CENTURION_BATTERY))
        .then(|| devices::battery(dev).ok())
        .flatten()
        .map(|(percent, charging)| Battery {
            percent,
            charging,
            hours: None,
        });

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
        fallback_picture: driver.and_then(crate::drivers::Registry::fallback_picture),
        picture_clip: driver.and_then(|d| d.layout.picture_clip.clone()),
        photo: driver.and_then(|d| d.layout.photo.clone()),
        off: false,
    }
}

/// The device's picture from the user's G HUB folders, as a data URL. Front views first,
/// dark variants preferred.
pub fn image(model: &str) -> Option<String> {
    let index = crate::assets::current();
    // Product views only; splash screens and promo cards are marketing art, not the device.
    const PREFERRED: [&str; 6] = [
        "front.png",
        "front_black.webp",
        "front_graphite.webp",
        "images/device/front_graphite.webp",
        "device_black.webp",
        "front_white.webp",
    ];
    let path = PREFERRED.iter().find_map(|f| index.device_file(model, f))?;
    let bytes = std::fs::read(&path).ok()?;
    let mime = format!("image/{}", crate::assets::image_type(&path)?);
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

/// A driver's product photo with its LED regions. The photo is downloaded once into the
/// cache; None when it can't be fetched (offline) or doesn't match.
pub fn photo_layout(model: &str, photo: &crate::drivers::Photo) -> Option<serde_json::Value> {
    use serde_json::json;
    let dir = std::path::PathBuf::from(std::env::var_os("HOME")?)
        .join(".cache/glowden/photos")
        .join(model);
    let file = dir.join("product-photo.png");
    let source = dir.join("product-photo.url");
    // Fetched again when the driver points at a different picture.
    let current = std::fs::read_to_string(&source).is_ok_and(|u| u.trim() == photo.url);
    if !file.exists() || !current {
        std::fs::create_dir_all(&dir).ok()?;
        let tmp = file.with_extension("part");
        let ok = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "30", "-A", "Mozilla/5.0", "-o"])
            .arg(&tmp)
            .arg(&photo.url)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("photo: cannot download {}", photo.url);
            let _ = std::fs::remove_file(&tmp);
            return None;
        }
        std::fs::rename(&tmp, &file).ok()?;
        let _ = std::fs::write(&source, &photo.url);
    }
    let bytes = std::fs::read(&file).ok()?;
    let (w, h) = image_size(&bytes)?;
    let [x, y, cw, ch] = photo.crop;
    if x + cw > w || y + ch > h {
        eprintln!(
            "photo: {} is {w}x{h}, smaller than its crop",
            file.display()
        );
        return None;
    }
    let shapes: Vec<_> = photo
        .leds
        .iter()
        .map(|l| json!({ "zone": l.zone, "shape": { "type": "path", "d": l.path } }))
        .collect();
    Some(json!({
        "width": cw,
        "height": ch,
        "image": format!("data:image/png;base64,{}", base64(&bytes)),
        "shapes": shapes,
        "legends": false,
        "crop": { "x": x, "y": y, "width": cw, "height": ch },
        "clip": null,
        "cutout": photo.cutout,
    }))
}

/// Device picture plus LED shapes in its pixel space, from the imported G HUB metadata.
/// Shape ids are zone ids, except on keyboards where they are HID usages.
pub fn layout(
    model: &str,
    keyboard: bool,
    first_view: bool,
    fallback: Option<&std::path::Path>,
    fallback_clip: Option<&str>,
) -> Option<serde_json::Value> {
    use serde_json::{Value, json};
    let index = crate::assets::current();
    let meta: Value =
        serde_json::from_slice(&std::fs::read(index.device_file(model, "metadata.json")?).ok()?)
            .ok()?;
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
    let mut candidates = index.pictures(model);
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
    let (path, bytes, drawn) = candidates
        .into_iter()
        .find_map(|p| {
            let b = std::fs::read(&p).ok()?;
            // Pictures can differ from the geometry's stated size by a pixel or two.
            let (iw, ih) = image_size(&b)?;
            (iw.abs_diff(w) <= w / 100 + 2 && ih.abs_diff(h) <= h / 100 + 2).then_some((p, b))
        })
        // The driver's own drawing, made at the geometry's size.
        .map(|(p, b)| (p, b, false))
        .or_else(|| fallback.and_then(|p| Some((p.to_path_buf(), std::fs::read(p).ok()?, true))))?;
    let mime = match crate::assets::image_type(&path) {
        Some(t) => format!("image/{t}"),
        None => "image/svg+xml".to_string(),
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
        "clip": if drawn { fallback_clip } else { None },
    }))
}

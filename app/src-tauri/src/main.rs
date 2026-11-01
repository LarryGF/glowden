use hidpp::devices::{self, Effect, mic};
use hidpp::discover::{self, Found};
use hidpp::features;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct FeatureView {
    id: String,
    name: String,
    version: u8,
}

#[derive(Serialize)]
struct Battery {
    percent: u8,
    charging: bool,
}

#[derive(Serialize)]
struct DeviceView {
    id: String,
    name: String,
    kind: &'static str,
    product: String,
    connection: &'static str,
    battery: Option<Battery>,
    /// Capabilities the UI can offer, derived from the feature list.
    caps: Vec<&'static str>,
    features: Vec<FeatureView>,
}

fn view(mut f: Found) -> DeviceView {
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

    let battery = if has(features::UNIFIED_BATTERY) {
        devices::battery(dev)
            .ok()
            .map(|(percent, charging)| Battery { percent, charging })
    } else {
        None
    };

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
        id: f.id,
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

type CmdResult<T> = Result<T, String>;

/// Device I/O blocks, so every command runs off the UI thread.
async fn with_device<T: Send + 'static>(
    id: String,
    f: impl FnOnce(&mut hidpp::Device) -> hidpp::Result<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut found = discover::open_id(&id).ok_or_else(|| format!("device {id} not found"))?;
        f(&mut found.device).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_devices() -> CmdResult<Vec<DeviceView>> {
    tauri::async_runtime::spawn_blocking(|| discover::open_all().into_iter().map(view).collect())
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum EffectArg {
    Solid {
        color: String,
    },
    Breathe {
        color: String,
        period_ms: u16,
        intensity: u8,
    },
    Cycle {
        period_ms: u16,
        intensity: u8,
        saturation: u8,
    },
}

fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16)
        .ok()
        .filter(|_| s.len() == 6)
        .ok_or("color must be #RRGGBB")?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[tauri::command]
async fn set_effect(id: String, effect: EffectArg) -> CmdResult<()> {
    let effect = match effect {
        EffectArg::Solid { color } => Effect::Solid {
            rgb: parse_rgb(&color)?,
        },
        EffectArg::Breathe {
            color,
            period_ms,
            intensity,
        } => Effect::Breathe {
            rgb: parse_rgb(&color)?,
            period_ms,
            intensity,
        },
        EffectArg::Cycle {
            period_ms,
            intensity,
            saturation,
        } => Effect::Cycle {
            period_ms,
            intensity,
            saturation,
        },
    };
    with_device(id, move |d| devices::set_effect(d, effect)).await
}

#[tauri::command]
async fn set_brightness(id: String, percent: u8) -> CmdResult<()> {
    with_device(id, move |d| devices::set_brightness(d, percent)).await
}

#[tauri::command]
async fn get_gain(id: String) -> CmdResult<u8> {
    with_device(id, mic::gain).await
}

#[tauri::command]
async fn set_gain(id: String, gain: u8) -> CmdResult<()> {
    with_device(id, move |d| mic::set_gain(d, gain)).await
}

#[tauri::command]
async fn set_muted(id: String, muted: bool) -> CmdResult<()> {
    with_device(id, move |d| mic::set_muted(d, muted)).await
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_devices,
            set_effect,
            set_brightness,
            get_gain,
            set_gain,
            set_muted
        ])
        .run(tauri::generate_context!())
        .expect("failed to start lhub");
}

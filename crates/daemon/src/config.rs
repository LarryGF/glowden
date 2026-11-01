//! Persistent per-device settings, re-applied whenever a device (re)appears.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Lighting as the user configured it. Speeds and sizes are 1..=100 sliders; the worker
/// converts them to device or engine units.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Lighting {
    /// Device-native effects: the device keeps running them without the service.
    Fixed { color: String },
    Breathing {
        color: String,
        speed: u8,
        intensity: u8,
    },
    Cycle {
        speed: u8,
        intensity: u8,
        saturation: u8,
    },
    /// Keyboard firmware ripple on key press (G515 effect 0x0017).
    Ripple { color: String },
    /// Host-rendered per-key effects: they need the service running.
    Wave { speed: u8 },
    Pulse {
        color: String,
        base: String,
        speed: u8,
        width: u8,
        fade: u8,
        rainbow: bool,
    },
    Starlight {
        star: String,
        sky: String,
        density: u8,
    },
    Visualizer {
        low: String,
        high: String,
        background: String,
    },
}

impl Lighting {
    pub fn is_host_rendered(&self) -> bool {
        matches!(
            self,
            Lighting::Wave { .. }
                | Lighting::Pulse { .. }
                | Lighting::Starlight { .. }
                | Lighting::Visualizer { .. }
        )
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct DeviceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    /// Keyed by stable device id ("<usb port>:<product>").
    pub devices: BTreeMap<String, DeviceConfig>,
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    base.join("lhub/config.json")
}

impl Config {
    pub fn load() -> Config {
        match std::fs::read(path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                eprintln!("config: ignoring unreadable {}: {e}", path().display());
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    /// Writes atomically so a crash mid-write can't leave a truncated file.
    pub fn save(&self) {
        let p = path();
        let tmp = p.with_extension("json.tmp");
        let result = std::fs::create_dir_all(p.parent().unwrap())
            .and_then(|_| std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap()))
            .and_then(|_| std::fs::rename(&tmp, &p));
        if let Err(e) = result {
            eprintln!("config: cannot save {}: {e}", p.display());
        }
    }
}

pub fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16)
        .ok()
        .filter(|_| s.len() == 6)
        .ok_or_else(|| format!("bad color '{s}', expected #RRGGBB"))?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// 1..=100 slider → value in [lo, hi].
pub fn lerp(slider: u8, lo: f32, hi: f32) -> f32 {
    lo + (slider.clamp(1, 100) as f32 - 1.0) / 99.0 * (hi - lo)
}

/// Speed slider → cycle period in ms (faster = shorter), for device-native effects.
pub fn period_ms(speed: u8) -> u16 {
    lerp(speed, 20000.0, 1000.0) as u16
}

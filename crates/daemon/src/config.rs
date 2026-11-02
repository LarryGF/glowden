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
    /// Effect stored in the device firmware, defined by the device's driver file.
    Firmware {
        id: String,
        color: String,
        #[serde(default = "full")]
        glow: u8,
    },
    /// Older configs: the G515 ripple before firmware effects came from driver files.
    Ripple { color: String },
    /// Per-key painting: every zone gets `base` unless `keys` (zone id → color) says otherwise.
    /// Sent once as a static frame.
    Freestyle {
        base: String,
        #[serde(default)]
        keys: BTreeMap<u8, String>,
    },
    /// Host-rendered per-key effects: they need the service running.
    Wave { speed: u8 },
    Pulse {
        color: String,
        base: String,
        speed: u8,
        width: u8,
        fade: u8,
        rainbow: bool,
        #[serde(default = "full")]
        glow: u8,
        #[serde(default = "full")]
        backdrop: u8,
    },
    Starlight {
        star: String,
        sky: String,
        density: u8,
        #[serde(default = "full")]
        glow: u8,
        #[serde(default = "full")]
        backdrop: u8,
    },
    Visualizer {
        low: String,
        high: String,
        background: String,
        #[serde(default = "full")]
        glow: u8,
        #[serde(default = "full")]
        backdrop: u8,
    },
}

impl Lighting {
    /// Firmware effects and Freestyle belong to one device and are left out of sync.
    pub fn is_syncable(&self) -> bool {
        !matches!(
            self,
            Lighting::Firmware { .. } | Lighting::Ripple { .. } | Lighting::Freestyle { .. }
        )
    }

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
    /// Mouse DPI presets (G HUB allows up to 5) and which one is active.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dpi_levels: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi_level: Option<usize>,
    /// Lighting power switch; off keeps `lighting` for when it's switched back on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power: Option<bool>,
    /// Report interval in ms (1 = 1000 Hz).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_interval: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceConfig>,
    /// From the app settings when applied, not saved per device.
    #[serde(skip)]
    pub swap_buttons: bool,
    /// Button number (as the mouse's button spy counts them, from 1) → what it does.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assignments: BTreeMap<u8, Action>,
}

/// What an assigned button does. Keys use Linux names (`KEY_LEFTCTRL`, `BTN_SIDE`, see
/// keys.rs); held keys and buttons stay down while the button is held.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// A key or shortcut, e.g. ["KEY_LEFTCTRL", "KEY_C"].
    Keys {
        keys: Vec<String>,
    },
    /// Keys pressed and released in order, with optional pauses.
    Macro {
        steps: Vec<MacroStep>,
    },
    MouseButton {
        button: String,
    },
    /// One wheel notch per press: "up", "down", "left" or "right".
    Scroll {
        direction: String,
    },
    /// "up", "down", "cycle" or "shift" (lowest level while held).
    Dpi {
        action: String,
    },
    /// A program started on press, through `sh -c`.
    Command {
        command: String,
    },
    /// A sound file played on press (G HUB's sound clips are imported into
    /// ~/.local/share/lhub/sounds).
    Sound {
        file: String,
    },
    /// The button does nothing.
    Disabled,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum MacroStep {
    /// Press and release one key or shortcut.
    Keys {
        keys: Vec<String>,
    },
    Pause {
        ms: u32,
    },
}

/// Blue VO!CE: the virtual microphone and the preset it runs.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct VoiceConfig {
    pub enabled: bool,
    /// Make the processed microphone the default input while VO!CE is on.
    #[serde(default = "yes")]
    pub default_mic: bool,
    /// Preset name as imported from G HUB; the first preset when unset or missing.
    pub preset: Option<String>,
    /// Fine-tuning on top of the preset: preset field path → value. Cleared when the preset
    /// changes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, serde_json::Value>,
    /// Voice effect preset (G HUB's voice effects); none or "Off" means no effects. Effects
    /// work with VO!CE off too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// Fine-tuning on top of the effect preset, as `overrides`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub effect_overrides: BTreeMap<String, serde_json::Value>,
}

impl VoiceConfig {
    pub fn effect_on(&self) -> bool {
        self.effect.as_deref().is_some_and(|e| e != "Off")
    }

    /// Whether the virtual microphone runs at all.
    pub fn active(&self) -> bool {
        self.enabled || self.effect_on()
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    /// Keyed by stable device id ("<usb port>:<product>").
    pub devices: BTreeMap<String, DeviceConfig>,
    #[serde(default)]
    pub app: AppSettings,
    /// Per-app profiles; the settings above are the default profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
    /// Switch profiles with the focused app.
    #[serde(default = "yes")]
    pub profile_auto: bool,
    /// Profile picked by hand (used while automatic switching is off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    /// Lighting saved by the user to reuse on any device.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lighting_presets: Vec<LightingPreset>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LightingPreset {
    pub name: String,
    pub lighting: Lighting,
}

/// A profile changes some device settings while active: lighting, brightness, DPI, report
/// rate and button assignments. Anything it doesn't set comes from the default profile.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub name: String,
    /// Window classes (as KWin reports them, e.g. "steam_app_570", "firefox").
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceConfig>,
}

impl Config {
    /// A device's settings with the active profile's changes on top.
    pub fn effective(&self, id: &str, active: Option<&str>) -> DeviceConfig {
        let mut cfg = self.devices.get(id).cloned().unwrap_or_default();
        cfg.swap_buttons = self.app.swap_mouse_buttons;
        let Some(over) = active
            .and_then(|name| self.profiles.iter().find(|p| p.name == name))
            .and_then(|p| p.devices.get(id))
        else {
            return cfg;
        };
        if over.lighting.is_some() {
            cfg.lighting = over.lighting.clone();
        }
        if over.power.is_some() {
            cfg.power = over.power;
        }
        if over.brightness.is_some() {
            cfg.brightness = over.brightness;
        }
        if !over.dpi_levels.is_empty() {
            cfg.dpi_levels = over.dpi_levels.clone();
        }
        if over.dpi_level.is_some() {
            cfg.dpi_level = over.dpi_level;
        }
        if over.report_interval.is_some() {
            cfg.report_interval = over.report_interval;
        }
        if !over.assignments.is_empty() {
            cfg.assignments = over.assignments.clone();
        }
        cfg
    }
}

/// Settings of lhub itself (the app's Settings page).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppSettings {
    /// Folders with G HUB data (depots, settings.db) to import pictures, the VO!CE engine
    /// and presets from.
    #[serde(default)]
    pub ghub_paths: Vec<String>,
    #[serde(default = "yes")]
    pub low_battery_alert: bool,
    /// Lights off while the session is locked, back on when unlocked (G HUB `lighting_sleep`).
    #[serde(default = "yes")]
    pub lighting_off_when_locked: bool,
    /// Left-handed: swap the mice's left and right buttons (G HUB `mouse_buttons_swapped`).
    #[serde(default)]
    pub swap_mouse_buttons: bool,
    /// Red breathing on devices with a low battery (G HUB's low-battery effect).
    #[serde(default = "yes")]
    pub low_battery_lighting: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            ghub_paths: Vec::new(),
            low_battery_alert: true,
            lighting_off_when_locked: true,
            swap_mouse_buttons: false,
            low_battery_lighting: true,
        }
    }
}

fn yes() -> bool {
    true
}

pub fn path() -> PathBuf {
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

fn full() -> u8 {
    100
}

/// Color from config, scaled by a 0..=100 layer level.
pub fn layer_rgb(hex: &str, level: u8) -> Result<[u8; 3], String> {
    let k = level.min(100) as u32;
    Ok(parse_rgb(hex)?.map(|c| (c as u32 * k / 100) as u8))
}

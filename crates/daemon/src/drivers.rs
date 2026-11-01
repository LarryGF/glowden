//! Device definitions ("drivers"): what isn't discoverable from HID++ itself, as data.
//!
//! Standard features (RGB effects, per-key lighting, DPI, report rate, battery, mic) work for
//! any device that reports them. A definition file adds the rest: kind, picture model,
//! key layout, reactive input and firmware-only effects. See docs/adding-devices.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Driver {
    pub name: String,
    pub kind: Option<String>,
    pub products: Vec<u16>,
    pub ghub_model: Option<String>,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub input: Input,
    #[serde(default, rename = "firmware_effect")]
    pub firmware_effects: Vec<FirmwareEffect>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Layout {
    /// Built-in key geometry ("iso-tkl"); zones are laid out in a line otherwise.
    pub keys: Option<String>,
    /// "hid-usage" when zone ids are USB key codes − 3 (G515); zone ids are used as-is otherwise.
    pub zones: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Input {
    /// Read key presses from the device's input node for reactive effects.
    #[serde(default)]
    pub reactive: bool,
}

/// An effect that runs in the device firmware, addressed by effect id on a cluster.
#[derive(Deserialize, Clone, Debug)]
pub struct FirmwareEffect {
    pub id: String,
    pub label: String,
    pub cluster: u8,
    pub effect_id: u16,
    /// Up to 10 parameter bytes as hex; `{r}{g}{b}` are replaced with the chosen color.
    pub params: String,
}

impl FirmwareEffect {
    pub fn params_for(&self, rgb: [u8; 3]) -> Result<Vec<u8>, String> {
        let hex = self
            .params
            .replace("{r}", &format!("{:02x}", rgb[0]))
            .replace("{g}", &format!("{:02x}", rgb[1]))
            .replace("{b}", &format!("{:02x}", rgb[2]));
        if hex.len() % 2 != 0 || hex.len() > 20 {
            return Err(format!(
                "firmware effect {}: params must be up to 10 hex bytes",
                self.id
            ));
        }
        (0..hex.len() / 2)
            .map(|i| {
                u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                    .map_err(|e| format!("firmware effect {}: {e}", self.id))
            })
            .collect()
    }
}

/// Where definitions are read from, lowest priority first. Later files override earlier ones
/// for the same product, so a user file can patch a shipped definition.
fn search_path() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/lhub/drivers")];
    // Development builds: <repo>/target/<profile>/lhubd → <repo>/drivers.
    if let Ok(exe) = std::env::current_exe()
        && let Some(repo) = exe.parent().and_then(Path::parent).and_then(Path::parent)
    {
        dirs.push(repo.join("drivers"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/lhub/drivers"));
    }
    if let Some(extra) = std::env::var_os("LHUB_DRIVERS") {
        dirs.extend(std::env::split_paths(&extra));
    }
    dirs
}

pub struct Registry {
    by_product: HashMap<u16, Driver>,
}

impl Registry {
    pub fn load() -> Registry {
        let mut by_product = HashMap::new();
        for dir in search_path() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut files: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "toml"))
                .collect();
            files.sort();
            for file in files {
                let parsed = std::fs::read_to_string(&file)
                    .map_err(|e| e.to_string())
                    .and_then(|s| toml::from_str::<Driver>(&s).map_err(|e| e.to_string()));
                match parsed {
                    Ok(d) => {
                        eprintln!("driver: {} from {}", d.name, file.display());
                        for &p in &d.products {
                            by_product.insert(p, d.clone());
                        }
                    }
                    Err(e) => eprintln!("driver: skipping {}: {e}", file.display()),
                }
            }
        }
        Registry { by_product }
    }

    pub fn for_product(&self, product: u16) -> Option<&Driver> {
        self.by_product.get(&product)
    }
}

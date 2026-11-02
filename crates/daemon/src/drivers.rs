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
    pub voice: Option<VoiceDef>,
    /// Names for the mouse's buttons, by button-spy number.
    #[serde(default, rename = "button")]
    pub buttons: Vec<ButtonDef>,
    /// Directory of the definition file, for paths inside it.
    #[serde(skip)]
    pub dir: PathBuf,
    #[serde(skip)]
    pub file: PathBuf,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Layout {
    /// Built-in key geometry ("iso-tkl"); zones are laid out in a line otherwise.
    pub keys: Option<String>,
    /// "hid-usage" when zone ids are USB key codes − 3 (G515); zone ids are used as-is otherwise.
    pub zones: Option<String>,
    /// "first" crops multi-view lighting pictures to their first view (Yeti GX front).
    pub lighting_view: Option<String>,
    /// Picture used when no imported one matches the LED geometry, relative to the file.
    pub picture: Option<String>,
    /// SVG path limiting where light shows on `picture`.
    pub picture_clip: Option<String>,
    /// A product photo downloaded on first use, with its LED regions.
    pub photo: Option<Photo>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Photo {
    pub url: String,
    /// x, y, width, height of the device in the downloaded picture.
    pub crop: [u32; 4],
    /// "vivid": strongly colored pixels inside LED regions show the live colors.
    pub cutout: Option<String>,
    #[serde(default, rename = "led")]
    pub leds: Vec<LedRegion>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LedRegion {
    pub zone: u8,
    /// SVG path in cropped-photo pixels.
    pub path: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ButtonDef {
    pub number: u8,
    pub label: String,
}

/// Blue VO!CE for this device: which PipeWire source the engine processes.
#[derive(Deserialize, Clone, Debug)]
pub struct VoiceDef {
    /// Part of the PipeWire node name of the device's microphone.
    pub source: String,
    /// Name the virtual microphone gets in sound settings.
    pub description: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Input {
    /// Read key presses from the device's input node for reactive effects.
    #[serde(default)]
    pub reactive: bool,
    /// Button-spy masks (0x8110) of buttons the host handles while the mouse is in host mode.
    pub dpi_up: Option<u16>,
    pub dpi_down: Option<u16>,
    /// Held: lowest DPI level; released: back to the current level.
    pub dpi_shift: Option<u16>,
    /// What each spy-numbered button sends to the PC when unassigned (0 = handled by the
    /// host). Factory routing (button n → n) when unset.
    pub routing: Option<Vec<u8>>,
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
                    Ok(mut d) => {
                        d.dir = dir.clone();
                        d.file = file.clone();
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

    pub fn fallback_picture(d: &Driver) -> Option<PathBuf> {
        d.layout.picture.as_ref().map(|p| d.dir.join(p))
    }

    /// Every loaded definition once (a file can list several products).
    pub fn all(&self) -> Vec<&Driver> {
        let mut v: Vec<&Driver> = self.by_product.values().collect();
        v.sort_by(|a, b| a.file.cmp(&b.file));
        v.dedup_by(|a, b| a.file == b.file);
        v
    }

    pub fn for_product(&self, product: u16) -> Option<&Driver> {
        self.by_product.get(&product)
    }
}

/// Writes a starter definition for a connected device into the user's driver folder.
pub fn create_starter(name: &str, kind: &str, product: u16) -> Result<PathBuf, String> {
    let dir = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
        .join(".local/share/lhub/drivers");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let file = dir.join(format!("{slug}.toml"));
    if file.exists() {
        return Err(format!("{} already exists", file.display()));
    }
    let model = slug.replace('-', "_");
    let text = format!(
        "# Starter definition written by lhub. See docs/adding-devices.md for pictures, LED\n\
         # positions, host-handled buttons and firmware effects.\n\
         name = \"{name}\"\n\
         kind = \"{kind}\"\n\
         products = [0x{product:04x}]\n\
         # Folder of its pictures in ~/.local/share/lhub/devices/ (G HUB's model name).\n\
         ghub_model = \"{model}\"\n"
    );
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    Ok(file)
}

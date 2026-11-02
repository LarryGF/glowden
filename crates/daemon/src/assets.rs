//! Files from a G HUB installation the user points GlowDen at (Settings → G HUB folders): a
//! mounted Windows drive, a G HUB folder, or any folder holding the files. They are read
//! where they are, never copied; this module only remembers where things were found.
//!
//! Found by name: device resource packages (a `manifest.json` next to a `metadata.json`),
//! the sound clip package (`audio_samples`), G HUB's settings (`settings.db` or an exported
//! `settings.json`, with the VO!CE presets) and the Blue VO!CE engine DLL.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const ENGINE_DLL: &str = "logi_audio_dts_clearvoice_2_capture_apo.dll";

#[derive(Default, Debug)]
pub struct Index {
    /// Device resource packages: (package folder, its manifest text).
    packages: Vec<(PathBuf, String)>,
    /// The sound clip package's manifest.
    pub sounds: Option<PathBuf>,
    /// G HUB's settings, newest first.
    pub settings: Vec<PathBuf>,
    pub engine: Option<PathBuf>,
}

static INDEX: Mutex<Option<(Vec<String>, Arc<Index>)>> = Mutex::new(None);
static ROOTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The folders to read from (the app settings' G HUB folders).
pub fn set_roots(roots: &[String]) {
    *ROOTS.lock().unwrap() = roots.to_vec();
}

/// The index of the current folders, scanned on first use and whenever the list changes.
pub fn current() -> Arc<Index> {
    let roots = ROOTS.lock().unwrap().clone();
    let mut cached = INDEX.lock().unwrap();
    if let Some((r, index)) = cached.as_ref()
        && *r == roots
    {
        return index.clone();
    }
    let index = Arc::new(scan(&roots));
    *cached = Some((roots, index.clone()));
    index
}

/// Forgets the index, so the next use scans the folders again.
pub fn rescan() {
    *INDEX.lock().unwrap() = None;
}

/// Where G HUB keeps its files inside a Windows installation, for folders that are a whole
/// Windows drive (scanning all of it would take minutes).
fn windows_places(root: &Path) -> Option<Vec<PathBuf>> {
    let windows = ["Windows", "windows"]
        .iter()
        .find(|w| root.join(w).is_dir())?;
    let mut places = vec![
        root.join("ProgramData/LGHUB"),
        root.join("Program Files/LGHUB"),
        root.join(windows)
            .join("System32/DriverStore/FileRepository"),
    ];
    for user in std::fs::read_dir(root.join("Users"))
        .into_iter()
        .flatten()
        .flatten()
    {
        places.push(user.path().join("AppData/Local/LGHUB"));
    }
    Some(places)
}

fn scan(roots: &[String]) -> Index {
    let mut index = Index::default();
    for root in roots {
        let root = PathBuf::from(shellexpand(root));
        let places = windows_places(&root).unwrap_or_else(|| vec![root.clone()]);
        for place in places {
            walk(&place, 0, &mut index);
        }
    }
    index
        .settings
        .sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    eprintln!(
        "G HUB folders: {} device packages, sounds {}, settings {}, VO!CE engine {}",
        index.packages.len(),
        if index.sounds.is_some() {
            "found"
        } else {
            "not found"
        },
        if index.settings.is_empty() {
            "not found"
        } else {
            "found"
        },
        if index.engine.is_some() {
            "found"
        } else {
            "not found"
        },
    );
    index
}

fn shellexpand(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => p.to_string(),
    }
}

fn walk(dir: &Path, depth: usize, index: &mut Index) {
    const SKIP: [&str; 5] = [
        "node_modules",
        "WinSxS",
        "$Recycle.Bin",
        "System Volume Information",
        ".git",
    ];
    if depth > 14 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                walk(&path, depth + 1, index);
            }
            continue;
        }
        match name.as_str() {
            "manifest.json" if dir.join("metadata.json").exists() => {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                index.packages.push((dir.to_path_buf(), text));
            }
            "manifest.json" if dir.file_name().is_some_and(|n| n == "audio_samples") => {
                index.sounds = Some(path);
            }
            "settings.db" | "settings.json" => index.settings.push(path),
            n if n.eq_ignore_ascii_case(ENGINE_DLL) => index.engine = Some(path),
            _ => {}
        }
    }
}

impl Index {
    /// The resource package for a device, by G HUB's model name.
    pub fn package(&self, model: &str) -> Option<&Path> {
        let quoted = format!("\"{model}\"");
        self.packages
            .iter()
            .find(|(_, text)| text.contains(&quoted))
            .map(|(dir, _)| dir.as_path())
    }

    /// A file in a device's package by its path there. Pictures are sometimes stored without
    /// an extension; "front.png" also matches an extensionless "front" holding a PNG.
    pub fn device_file(&self, model: &str, rel: &str) -> Option<PathBuf> {
        let dir = self.package(model)?;
        let exact = dir.join(rel);
        if exact.is_file() {
            return Some(exact);
        }
        let bare = exact.with_extension("");
        let want = Path::new(rel).extension()?.to_str()?;
        (bare.is_file() && image_type(&bare) == Some(want)).then_some(bare)
    }

    /// Every PNG or WebP picture in a device's package.
    pub fn pictures(&self, model: &str) -> Vec<PathBuf> {
        let Some(dir) = self.package(model) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if image_type(&p).is_some() {
                    out.push(p);
                }
            }
        }
        out
    }
}

/// "png" or "webp" from a file's first bytes.
pub fn image_type(path: &Path) -> Option<&'static str> {
    use std::io::Read;
    let mut head = [0u8; 12];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    if head.starts_with(b"\x89PNG") {
        Some("png")
    } else if head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

static SETTINGS: Mutex<Option<(PathBuf, std::time::SystemTime, Arc<serde_json::Value>)>> =
    Mutex::new(None);

/// G HUB's settings JSON: from `settings.db` (SQLite; its newest `data` row) or an exported
/// `settings.json`. Parsed again only when the file changes.
pub fn ghub_settings() -> Option<Arc<serde_json::Value>> {
    let path = current().settings.first()?.clone();
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
    let mut cached = SETTINGS.lock().unwrap();
    if let Some((p, m, v)) = cached.as_ref()
        && *p == path
        && *m == modified
    {
        return Some(v.clone());
    }
    let bytes = if path.extension().is_some_and(|e| e == "db") {
        let out = std::process::Command::new("sqlite3")
            .arg("-readonly")
            .arg(&path)
            .arg("select file from data order by _id desc limit 1")
            .output()
            .map_err(|e| eprintln!("reading {} needs the sqlite3 tool: {e}", path.display()))
            .ok()?;
        out.stdout
    } else {
        std::fs::read(&path).ok()?
    };
    let value: Arc<serde_json::Value> = Arc::new(serde_json::from_slice(&bytes).ok()?);
    *cached = Some((path, modified, value.clone()));
    Some(value)
}

/// The settings cards of one kind (e.g. `BLUE_VOICE_2_SETTINGS`), in G HUB's order.
pub fn ghub_cards(attribute: &str) -> Vec<serde_json::Value> {
    let Some(settings) = ghub_settings() else {
        return Vec::new();
    };
    settings["cards"]["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["attribute"] == attribute)
        .cloned()
        .collect()
}

/// G HUB's sound clips (the `audio_samples` package) as ("Category / Name", file).
pub fn sound_clips() -> Vec<(String, PathBuf)> {
    let Some(manifest) = current().sounds.clone() else {
        return Vec::new();
    };
    let package = manifest.parent().unwrap_or(Path::new("/")).to_path_buf();
    let Ok(json) = std::fs::read(&manifest)
        .map_err(|_| ())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|_| ()))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in json["resources"].as_array().into_iter().flatten() {
        let (Some(key), Some(src)) = (r["key"].as_str(), r["src"].as_str()) else {
            continue;
        };
        // "PRESET_SOUND_FX_CATEGORY#Generic_Sample_AirHorn" → "Sound fx / AirHorn"
        let (category, name) = key.split_once('#').unwrap_or(("Sounds", key));
        let category = category
            .trim_start_matches("PRESET_")
            .trim_end_matches("_CATEGORY")
            .replace('_', " ");
        let category = category[..1].to_string() + &category[1..].to_lowercase();
        let name = name.rsplit("_Sample_").next().unwrap_or(name);
        let file = package.join(src);
        if file.is_file() {
            out.push((format!("{category} / {name}"), file));
        }
    }
    out.sort();
    out
}

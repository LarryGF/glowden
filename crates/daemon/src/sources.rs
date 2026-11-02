//! Device sources: where device definitions (data) and plugins (programs) come from.
//!
//! A source is an `index.json` (see glowden-devices/tools/make-index.py) at a URL or in a
//! local folder, with the files it lists next to it. Definitions install freely; a source's
//! plugins run only once the user trusts that source. GlowDen's own source is always listed.
//!
//! Installed files live in ~/.local/share/glowden/sources/<source key>/:
//!   index.json                  last fetched index
//!   devices/<id>/…              installed definitions (the driver registry reads these)
//!   plugins/<id>/<binary>       downloaded plugins, checked against the index's SHA-256

use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::SourceConfig;

pub const DEFAULT_SOURCE: &str =
    "https://github.com/larryGF/glowden-devices/releases/latest/download/index.json";

fn root() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/glowden/sources")
}

/// Folder name for a source: a short hash of its URL.
pub fn key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    digest[..6].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn dir(url: &str) -> PathBuf {
    root().join(key(url))
}

/// For development: GLOWDEN_SOURCE_OVERRIDE replaces GlowDen's own source (e.g. a local
/// `make-index.py` output folder).
pub fn location(url: &str) -> String {
    match std::env::var("GLOWDEN_SOURCE_OVERRIDE") {
        Ok(over) if url == DEFAULT_SOURCE => over,
        _ => url.to_string(),
    }
}

/// A file next to the source's index: fetched over HTTP(S) with curl, or read from disk.
fn fetch(url: &str, name: &str) -> Result<Vec<u8>, String> {
    let loc = location(url);
    let local = loc.strip_prefix("file://").unwrap_or(&loc);
    if !local.contains("://") {
        let base = Path::new(local);
        let base = if base.is_dir() {
            base
        } else {
            base.parent().unwrap_or(base)
        };
        return std::fs::read(base.join(name))
            .map_err(|e| format!("{}: {e}", base.join(name).display()));
    }
    let base = &loc[..loc.rfind('/').map_or(loc.len(), |i| i + 1)];
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "60"])
        .arg(format!("{base}{name}"))
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("cannot download {base}{name}"));
    }
    Ok(out.stdout)
}

/// Downloads the source's index and keeps a copy for offline use.
pub fn refresh(url: &str) -> Result<Value, String> {
    let bytes = fetch(url, "index.json")?;
    let index: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("bad index.json: {e}"))?;
    if index["format"] != 1 {
        return Err("this source needs a newer GlowDen".into());
    }
    std::fs::create_dir_all(dir(url)).map_err(|e| e.to_string())?;
    std::fs::write(dir(url).join("index.json"), &bytes).map_err(|e| e.to_string())?;
    Ok(index)
}

/// The last fetched index, without going online.
pub fn cached(url: &str) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(dir(url).join("index.json")).ok()?).ok()
}

/// Folders of every installed definition, for the driver registry.
pub fn installed_device_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for source in std::fs::read_dir(root()).into_iter().flatten().flatten() {
        for dev in std::fs::read_dir(source.path().join("devices"))
            .into_iter()
            .flatten()
            .flatten()
        {
            out.push(dev.path());
        }
    }
    out.sort();
    out
}

pub fn is_installed(url: &str, id: &str) -> bool {
    dir(url)
        .join("devices")
        .join(id)
        .join("device.toml")
        .exists()
}

fn safe(name: &str) -> Result<&str, String> {
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    ok.then_some(name)
        .ok_or_else(|| format!("bad name in index: {name}"))
}

/// Installs one device's definition from a source (data only; no code runs).
pub fn install_device(url: &str, id: &str) -> Result<(), String> {
    let index = cached(url).map_or_else(|| refresh(url), Ok)?;
    let dev = index["devices"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["id"] == id)
        .ok_or_else(|| format!("{id} isn't in this source"))?;
    let target = dir(url).join("devices").join(safe(id)?);
    let tmp = target.with_extension("part");
    let _ = std::fs::remove_dir_all(&tmp);
    for (rel, asset) in dev["files"].as_object().into_iter().flatten() {
        let asset = safe(asset.as_str().unwrap_or(""))?;
        // Paths inside the device folder only.
        if rel.split('/').any(|p| p.is_empty() || p == "..") {
            return Err(format!("bad file path in index: {rel}"));
        }
        let to = tmp.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&to, fetch(url, asset)?).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn uninstall_device(url: &str, id: &str) -> Result<(), String> {
    std::fs::remove_dir_all(dir(url).join("devices").join(safe(id)?)).map_err(|e| e.to_string())
}

/// The source and device id offering a definition for `product`, from cached indexes.
pub fn find_for_product(sources: &[SourceConfig], product: u16) -> Option<(String, String)> {
    sources.iter().find_map(|s| {
        let index = cached(&s.url)?;
        let dev = index["devices"].as_array()?.iter().find(|d| {
            d["products"]
                .as_array()
                .is_some_and(|p| p.iter().any(|x| x == product))
        })?;
        Some((s.url.clone(), dev["id"].as_str()?.to_string()))
    })
}

/// Which source an installed definition folder belongs to.
pub fn source_of(device_dir: &Path, sources: &[SourceConfig]) -> Option<SourceConfig> {
    let key = device_dir
        .parent()?
        .parent()?
        .file_name()?
        .to_str()?
        .to_string();
    sources.iter().find(|s| self::key(&s.url) == key).cloned()
}

/// The plugin program `id` from the source that provides the device, downloading it the first
/// time. Fails until the user trusts that source.
pub fn plugin(source: &SourceConfig, id: &str) -> Result<PathBuf, String> {
    let index = cached(&source.url).ok_or("the source's index isn't available")?;
    let entry = index["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["id"] == id)
        .ok_or_else(|| format!("the source doesn't provide the {id} plugin"))?;
    if !source.trusted {
        return Err(format!(
            "{} needs the \"{}\" plugin; allow plugins from this source in Settings → Device sources",
            id,
            entry["name"].as_str().unwrap_or(id)
        ));
    }
    let arch = format!("{}-linux", std::env::consts::ARCH);
    let bin = &entry["binaries"][&arch];
    let asset = safe(
        bin["file"]
            .as_str()
            .ok_or_else(|| format!("no {id} build for {arch}"))?,
    )?;
    let want = bin["sha256"].as_str().unwrap_or("").to_string();
    let path = dir(&source.url).join("plugins").join(safe(id)?).join(asset);
    let sum = |b: &[u8]| -> String {
        Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect()
    };
    if std::fs::read(&path).is_ok_and(|b| sum(&b) == want) {
        return Ok(path);
    }
    let bytes = fetch(&source.url, asset)?;
    if sum(&bytes) != want {
        return Err(format!(
            "the downloaded {id} plugin doesn't match its checksum"
        ));
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

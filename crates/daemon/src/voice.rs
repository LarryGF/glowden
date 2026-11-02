//! Blue VO!CE: runs `lhub-voice` (the user's own copy of the ClearVoice 2 engine as a
//! PipeWire virtual microphone) for devices whose driver file has a `[voice]` section.
//! Engine and presets are imported with `lhub import-ghub` / `lhub import-voice-presets`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde_json::Value;

use crate::drivers::VoiceDef;

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/lhub/voice")
}

pub fn engine() -> Option<PathBuf> {
    Some(dir().join("clearvoice2.dll")).filter(|p| p.exists())
}

/// (name, settings) of each imported preset, in G HUB's order.
pub fn presets() -> Vec<(String, Value)> {
    let Ok(bytes) = std::fs::read(dir().join("presets.json")) else {
        return Vec::new();
    };
    let list: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    list.into_iter()
        .filter_map(|p| Some((p["name"].as_str()?.to_string(), p["settings"].clone())))
        .collect()
}

/// Preset field → (offset in the engine's parameter block, 'f' float or 'b' byte).
/// Measured one field at a time with tools/voice-probes/field_probe; see docs/voice.md.
const FIELDS: &[(&str, u16, char)] = &[
    ("highPassFilter.enabled", 0x014, 'b'),
    ("highPassFilter.frequency", 0x010, 'f'),
    ("noiseReduction.release", 0x01c, 'f'),
    ("noiseReduction.sensitivity", 0x020, 'f'),
    ("noiseReduction.bias", 0x024, 'f'),
    ("noiseReduction.maxAttenuation", 0x028, 'f'),
    ("noiseReduction.enabled", 0x02c, 'b'),
    ("gatingCompressor.noiseGateThreshold", 0x034, 'f'),
    ("gatingCompressor.noiseGateMaxAttenuation", 0x038, 'f'),
    ("gatingCompressor.noiseGateAttack", 0x03c, 'f'),
    ("gatingCompressor.noiseGateRelease", 0x040, 'f'),
    ("gatingCompressor.noiseGateHoldTime", 0x044, 'f'),
    ("gatingCompressor.compressorThreshold", 0x048, 'f'),
    ("gatingCompressor.compressorRatio", 0x04c, 'f'),
    ("gatingCompressor.compressorPreGain", 0x050, 'f'),
    ("gatingCompressor.compressorPostGain", 0x054, 'f'),
    ("gatingCompressor.compressorAttack", 0x058, 'f'),
    ("gatingCompressor.compressorRelease", 0x05c, 'f'),
    ("gatingCompressor.compressorEnabled", 0x061, 'b'),
    ("multiBandEq.lowBandFilter.frequency", 0x070, 'f'),
    ("multiBandEq.lowBandFilter.gain", 0x074, 'f'),
    ("multiBandEq.lowBandFilter.q", 0x078, 'f'),
    ("multiBandEq.midBandFilter.frequency", 0x07c, 'f'),
    ("multiBandEq.midBandFilter.gain", 0x080, 'f'),
    ("multiBandEq.midBandFilter.q", 0x084, 'f'),
    ("multiBandEq.highBandFilter.frequency", 0x088, 'f'),
    ("multiBandEq.highBandFilter.gain", 0x08c, 'f'),
    ("multiBandEq.highBandFilter.q", 0x090, 'f'),
    ("multiBandEq.enabled", 0x09c, 'b'),
    ("dePopper.threshold", 0x0a4, 'f'),
    ("deEsser.threshold", 0x0a8, 'f'),
    ("dePopper.frequency", 0x0ac, 'f'),
    ("dePopper.width", 0x0b0, 'f'),
    ("dePopper.maxAttenuation", 0x0b4, 'f'),
    ("deEsser.frequency", 0x0b8, 'f'),
    ("deEsser.width", 0x0bc, 'f'),
    ("deEsser.maxAttenuation", 0x0c0, 'f'),
    ("dePopper.attack", 0x0c4, 'f'),
    ("dePopper.release", 0x0cc, 'f'),
    ("deEsser.attack", 0x0d0, 'f'),
    ("deEsser.release", 0x0d4, 'f'),
    ("dePopper.enabled", 0x0dd, 'b'),
    ("deEsser.enabled", 0x0de, 'b'),
    ("hardLimiter.attack", 0x140, 'f'),
    ("hardLimiter.release", 0x144, 'f'),
    ("hardLimiter.boost", 0x148, 'f'),
    ("hardLimiter.enabled", 0x150, 'b'),
];

/// `lhub-voice` arguments setting each preset field present in `settings`.
pub fn engine_args(settings: &Value) -> Vec<String> {
    let mut args = Vec::new();
    for &(path, offset, kind) in FIELDS {
        let v = path.split('.').fold(settings, |v, k| &v[k]);
        let value = match (kind, v) {
            ('b', Value::Bool(b)) => (*b as u8).to_string(),
            ('f', Value::Number(n)) => n.to_string(),
            _ => continue,
        };
        args.extend([kind.to_string(), format!("{offset:03x}"), value]);
    }
    args
}

/// The PipeWire source whose node name contains `pattern` (e.g. the Yeti GX's ALSA input).
fn source_node(pattern: &str) -> Option<String> {
    let out = Command::new("pw-dump")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let nodes: Vec<Value> = serde_json::from_slice(&out.stdout).ok()?;
    nodes.iter().find_map(|n| {
        let props = &n["info"]["props"];
        let name = props["node.name"].as_str()?;
        (props["media.class"] == "Audio/Source" && name.contains(pattern)).then(|| name.to_string())
    })
}

fn binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("lhub-voice")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("lhub-voice"))
}

/// What a preset does, for the app: the numbers that differ between presets.
pub fn summary(settings: &Value) -> Value {
    let n = |path: &str| path.split('.').fold(settings, |v, k| &v[k]).as_f64();
    serde_json::json!({
        "noise_reduction": n("noiseReduction.maxAttenuation"),
        "bass": n("multiBandEq.lowBandFilter.gain").unwrap_or(0.0),
        "mids": n("multiBandEq.midBandFilter.gain").unwrap_or(0.0),
        "treble": n("multiBandEq.highBandFilter.gain").unwrap_or(0.0),
        "compression": n("gatingCompressor.compressorRatio"),
        "de_esser": n("deEsser.maxAttenuation"),
        "boost": n("hardLimiter.boost"),
        "input_gain": n("inputGain"),
    })
}

fn test_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
        .join("lhub-voice-test")
}

/// Records `seconds` of the raw microphone for the preset test.
pub fn record_test(def: &VoiceDef, seconds: u64) -> Result<(), String> {
    let source = source_node(&def.source)
        .ok_or_else(|| format!("no PipeWire source matching \"{}\"", def.source))?;
    std::fs::create_dir_all(test_dir()).map_err(|e| e.to_string())?;
    let mut child = Command::new("pw-record")
        .args([
            "--target",
            &source,
            "--format",
            "f32",
            "--rate",
            "48000",
            "--channels",
            "1",
        ])
        .arg(test_dir().join("raw.wav"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start pw-record: {e}"))?;
    std::thread::sleep(std::time::Duration::from_secs(seconds.clamp(1, 30)));
    // SIGINT lets pw-record finish the WAV header.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let _ = child.wait();
    Ok(())
}

pub fn has_test() -> bool {
    test_dir().join("raw.wav").exists()
}

#[derive(Default)]
pub struct Voice {
    running: HashMap<String, Child>,
    playing: Option<Child>,
    /// Why the last start for a device failed, for the UI.
    errors: HashMap<String, String>,
}

impl Voice {
    pub fn start(&mut self, id: &str, def: &VoiceDef, settings: &Value) {
        self.stop(id);
        match Self::spawn(def, settings) {
            Ok(child) => {
                self.errors.remove(id);
                self.running.insert(id.to_string(), child);
            }
            Err(e) => {
                eprintln!("{id}: Blue VO!CE: {e}");
                self.errors.insert(id.to_string(), e);
            }
        }
    }

    fn spawn(def: &VoiceDef, settings: &Value) -> Result<Child, String> {
        let dll = engine().ok_or("the engine isn't imported yet (lhub import-ghub)")?;
        let source = source_node(&def.source)
            .ok_or_else(|| format!("no PipeWire source matching \"{}\"", def.source))?;
        Command::new(binary())
            .arg(dll)
            .args(["--target", &source, "--name", "lhub-bluevoice"])
            .args(["--description", &def.description])
            .args(engine_args(settings))
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start lhub-voice: {e}"))
    }

    /// Plays the test recording as recorded (`None`) or through a preset.
    pub fn play_test(&mut self, settings: Option<&Value>) -> Result<(), String> {
        self.stop_test();
        let raw = test_dir().join("raw.wav");
        if !raw.exists() {
            return Err("record a test first".into());
        }
        let file = match settings {
            None => raw,
            Some(s) => {
                let out = test_dir().join("processed.wav");
                let dll = engine().ok_or("the engine isn't imported yet")?;
                let status = Command::new(binary())
                    .arg(dll)
                    .args(engine_args(s))
                    .arg("--process")
                    .args([&raw, &out])
                    .stdin(Stdio::null())
                    .status()
                    .map_err(|e| format!("cannot start lhub-voice: {e}"))?;
                if !status.success() {
                    return Err("processing the test failed".into());
                }
                out
            }
        };
        self.playing = Some(
            Command::new("pw-play")
                .arg(file)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("cannot start pw-play: {e}"))?,
        );
        Ok(())
    }

    pub fn stop_test(&mut self) {
        if let Some(mut child) = self.playing.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn stop(&mut self, id: &str) {
        if let Some(mut child) = self.running.remove(id) {
            unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            let _ = child.wait();
        }
    }

    /// Whether the virtual microphone is up; a crashed host is noticed and reported.
    pub fn status(&mut self, id: &str) -> (bool, Option<String>) {
        if let Some(child) = self.running.get_mut(id)
            && let Ok(Some(code)) = child.try_wait()
        {
            self.running.remove(id);
            self.errors.insert(
                id.to_string(),
                format!("the virtual microphone stopped ({code})"),
            );
        }
        (self.running.contains_key(id), self.errors.get(id).cloned())
    }
}

/// Hosts left over from a previous service run (they outlive a killed service).
pub fn stop_stale() {
    for e in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim() == "lhub-voice") {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

//! Blue VO!CE: runs `glowden-voice` (the user's own copy of the ClearVoice 2 engine as a
//! PipeWire virtual microphone) for devices whose driver file has a `[voice]` section.
//! Engine and presets are imported with `glowdenctl import-ghub` / `glowdenctl import-voice-presets`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde_json::Value;

use crate::drivers::VoiceDef;

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/glowden/voice")
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

/// `glowden-voice` arguments setting each preset field present in `settings`.
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

/// Voice effect presets imported from G HUB: (name, settings), "Off" first.
pub fn effect_presets() -> Vec<(String, Value)> {
    let Ok(bytes) = std::fs::read(dir().join("effects.json")) else {
        return Vec::new();
    };
    let list: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    list.into_iter()
        .filter_map(|p| Some((p["name"].as_str()?.to_string(), p["settings"].clone())))
        .collect()
}

/// Voice effect stage → host section, the field enabling it, and its fields: (preset field,
/// offset, kind, enum names in value order). The layouts follow G HUB's field order, which
/// matches each stage's default parameters; offsets and the enable bytes were checked on a
/// test tone (tools/voice-probes/stage_probe). Fields G HUB leaves out are 0/false/the first
/// enum value, as in its settings format.
type EffectField = (&'static str, u16, char, &'static [&'static str]);
const EFFECTS: &[(&str, &str, Option<&str>, u16, &[EffectField])] = &[
    (
        "vocalChanger",
        "@vocal",
        None,
        0x30,
        &[
            ("vocalMode1", 0x00, 'u', VOCAL_MODES),
            ("voiceModFactor1", 0x04, 'f', &[]),
            ("vocalMode2", 0x08, 'u', VOCAL_MODES),
            ("voiceModFactor2", 0x0c, 'f', &[]),
            ("voiceGain1", 0x10, 'f', &[]),
            ("voiceGain2", 0x14, 'f', &[]),
            ("chorusEnabled", 0x18, 'b', &[]),
            ("chorusDelay", 0x1c, 'f', &[]),
            ("chorusDelayMod", 0x20, 'f', &[]),
            ("chorusDelayModRate", 0x24, 'f', &[]),
            ("chorusPitchMod", 0x28, 'f', &[]),
            ("chorusPitchModRate", 0x2c, 'f', &[]),
        ],
    ),
    (
        "flangerPhaser",
        "@flanger",
        Some("enabled"),
        0x10,
        &[
            ("phaserMode", 0x00, 'u', PHASER_MODES),
            ("depth", 0x04, 'f', &[]),
            ("rate", 0x08, 'f', &[]),
        ],
    ),
    (
        "ringModulation",
        "@ring",
        Some("enabled"),
        0x10,
        &[
            ("mix", 0x00, 'f', &[]),
            ("shapeMode", 0x04, 'u', SHAPE_MODES),
            ("carrierFrequency", 0x08, 'f', &[]),
        ],
    ),
    (
        "reverb",
        "@reverb",
        Some("enabled"),
        0x24,
        &[
            ("earlyLateMix", 0x00, 'f', &[]),
            ("reflectionMode", 0x04, 'u', REFLECTION_MODES),
            ("reflectionLowPassFrequency", 0x08, 'f', &[]),
            ("roomSize", 0x0c, 'f', &[]),
            ("mix", 0x1c, 'f', &[]),
        ],
    ),
    (
        "delay",
        "@delay",
        Some("enabled"),
        0x20,
        &[
            ("inputMode", 0x00, 'u', INPUT_MODES),
            ("feedbackMode", 0x04, 'u', FEEDBACK_MODES),
            ("feedbackTimbre", 0x08, 'u', FEEDBACK_TIMBRES),
            ("leftDelay", 0x0c, 'f', &[]),
            ("rightDelay", 0x10, 'f', &[]),
            ("feedback", 0x14, 'f', &[]),
            ("mix", 0x18, 'f', &[]),
        ],
    ),
];

pub const VOCAL_MODES: &[&str] = &[
    "VOCAL_MODE_OFF",
    "VOCAL_MODE_FORMANT_WARP",
    "VOCAL_MODE_PITCH_SHIFT",
    "VOCAL_MODE_FORMANT_SHIFT",
    "VOCAL_MODE_ROBOT",
    "VOCAL_MODE_WHISPEZ",
    "VOCAL_MODE_NOICE",
    "VOCAL_MODE_GROWL",
    "VOCAL_MODE_AIR",
    "VOCAL_MODE_ETHER",
    "VOCAL_MODE_HOUSEOTRON",
    "VOCAL_MODE_FUNKBOT",
    "VOCAL_MODE_TALKBOX",
    "VOCAL_MODE_SUBBY",
    "VOCAL_MODE_NO_TREBLE",
];
pub const PHASER_MODES: &[&str] = &[
    "PHASER_MODE_FLANGER_UP",
    "PHASER_MODE_FLANGER_DOWN",
    "PHASER_MODE_FLANGER_OSCILLATE",
    "PHASER_MODE_PHASER_STUN",
    "PHASER_MODE_PHASER_HEAVY",
    "PHASER_MODE_PHASER_KILL",
];
pub const SHAPE_MODES: &[&str] = &[
    "SHAPE_MODE_SINE",
    "SHAPE_MODE_TRIANGLE",
    "SHAPE_MODE_SAW_TOOTH",
    "SHAPE_MODE_SQUARE",
    "SHAPE_MODE_HALFWAVERECTIFIER",
];
pub const REFLECTION_MODES: &[&str] = &[
    "REFLECTION_MODE_NONE",
    "REFLECTION_MODE_TIGHT",
    "REFLECTION_MODE_MIDMIX",
    "REFLECTION_MODE_LARGEDIFF",
    "REFLECTION_MODE_ECHO",
    "REFLECTION_MODE_ECHO2",
    "REFLECTION_MODE_ECHO3",
    "REFLECTION_MODE_FLUTTER",
    "REFLECTION_MODE_ST_FLUTTER",
    "REFLECTION_MODE_ECHO_FLUTTER",
];
pub const INPUT_MODES: &[&str] = &[
    "INPUT_MODE_NORMAL",
    "INPUT_MODE_CROSSED",
    "INPUT_MODE_LEFT",
    "INPUT_MODE_RIGHT",
];
pub const FEEDBACK_MODES: &[&str] = &["FEEDBACK_MODE_NORMAL", "FEEDBACK_MODE_CROSSED"];
pub const FEEDBACK_TIMBRES: &[&str] = &[
    "FEEDBACK_TIMBRE_FULL",
    "FEEDBACK_TIMBRE_DARK",
    "FEEDBACK_TIMBRE_BRIGHT",
];

/// Host sections for the voice effects in `effects` (G HUB's effect settings format); a
/// stage that is off is left out, which bypasses it.
pub fn effect_args(effects: &Value) -> Vec<String> {
    let mut args = Vec::new();
    for &(stage, section, enabled, enable_at, fields) in EFFECTS {
        let o = &effects[stage];
        let on = match enabled {
            Some(key) => o[key].as_bool().unwrap_or(false),
            // The voice changer has no switch: it is on when a voice or the chorus is.
            None => {
                o["chorusEnabled"].as_bool().unwrap_or(false)
                    || ["vocalMode1", "vocalMode2"]
                        .iter()
                        .any(|k| o[k].as_str().is_some_and(|m| m != "VOCAL_MODE_OFF"))
            }
        };
        if !on {
            continue;
        }
        args.push(section.to_string());
        for &(name, offset, kind, names) in fields {
            let v = &o[name];
            let value = match kind {
                'u' => names
                    .iter()
                    .position(|n| v.as_str() == Some(n))
                    .unwrap_or(0)
                    .to_string(),
                'b' => (v.as_bool().unwrap_or(false) as u8).to_string(),
                _ => v.as_f64().unwrap_or(0.0).to_string(),
            };
            args.extend([kind.to_string(), format!("{offset:02x}"), value]);
        }
        args.extend(["b".into(), format!("{enable_at:02x}"), "1".into()]);
    }
    args
}

/// Every host argument for a device: VO!CE (when on) and the voice effects.
pub fn host_args(voice: Option<&Value>, effects: Option<&Value>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(v) = voice {
        args.push("@cv2".to_string());
        args.extend(engine_args(v));
    }
    if let Some(e) = effects {
        args.extend(effect_args(e));
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

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// G HUB processes the microphone itself, so every app hears VO!CE. Here the processed
/// microphone becomes the default input while it runs. It appears a moment after the host
/// starts, so this retries briefly.
pub fn make_default() {
    std::thread::spawn(|| {
        for _ in 0..30 {
            if pactl(&["set-default-source", "glowden-voice-mic"]).is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("Blue VO!CE: couldn't make the virtual microphone the default input");
    });
}

/// Hands the default input back to the real microphone if VO!CE had it.
pub fn release_default(def: &VoiceDef) {
    if pactl(&["get-default-source"]).as_deref() == Some("glowden-voice-mic")
        && let Some(source) = source_node(&def.source)
    {
        pactl(&["set-default-source", &source]);
    }
}

fn binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("glowden-voice")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("glowden-voice"))
}

/// Fine-tuning controls offered on top of a preset (G HUB's advanced VO!CE controls): preset
/// field, group, label, min, max, step, unit. Ranges stay inside what the engine was seen to
/// accept (tools/voice-probes/field_probe).
pub const CONTROLS: &[(&str, &str, &str, f64, f64, f64, &str)] = &[
    (
        "highPassFilter.frequency",
        "High-pass filter",
        "Cutoff",
        20.0,
        400.0,
        1.0,
        "Hz",
    ),
    (
        "noiseReduction.maxAttenuation",
        "Noise reduction",
        "Amount",
        -30.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "noiseReduction.sensitivity",
        "Noise reduction",
        "Sensitivity",
        0.0,
        40.0,
        1.0,
        "",
    ),
    (
        "noiseReduction.release",
        "Noise reduction",
        "Release",
        10.0,
        1000.0,
        5.0,
        "ms",
    ),
    (
        "gatingCompressor.noiseGateThreshold",
        "Noise gate",
        "Threshold",
        -80.0,
        -20.0,
        1.0,
        "dB",
    ),
    (
        "gatingCompressor.noiseGateMaxAttenuation",
        "Noise gate",
        "Depth",
        -60.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "gatingCompressor.compressorThreshold",
        "Compressor",
        "Threshold",
        -40.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "gatingCompressor.compressorRatio",
        "Compressor",
        "Ratio",
        2.0,
        10.0,
        0.5,
        ":1",
    ),
    (
        "gatingCompressor.compressorPostGain",
        "Compressor",
        "Make-up gain",
        0.0,
        10.0,
        0.5,
        "dB",
    ),
    (
        "multiBandEq.lowBandFilter.gain",
        "Voice EQ",
        "Bass",
        -12.0,
        12.0,
        0.5,
        "dB",
    ),
    (
        "multiBandEq.midBandFilter.gain",
        "Voice EQ",
        "Mids",
        -12.0,
        12.0,
        0.5,
        "dB",
    ),
    (
        "multiBandEq.highBandFilter.gain",
        "Voice EQ",
        "Treble",
        -12.0,
        12.0,
        0.5,
        "dB",
    ),
    (
        "deEsser.threshold",
        "De-esser",
        "Threshold",
        -60.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "deEsser.maxAttenuation",
        "De-esser",
        "Amount",
        -40.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "dePopper.threshold",
        "De-popper",
        "Threshold",
        -60.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "dePopper.maxAttenuation",
        "De-popper",
        "Amount",
        -40.0,
        0.0,
        1.0,
        "dB",
    ),
    (
        "hardLimiter.boost",
        "Limiter",
        "Boost",
        0.0,
        10.0,
        0.5,
        "dB",
    ),
];

/// Each stage's on/off switch, per group above.
pub const SWITCHES: &[(&str, &str)] = &[
    ("High-pass filter", "highPassFilter.enabled"),
    ("Noise reduction", "noiseReduction.enabled"),
    ("Compressor", "gatingCompressor.compressorEnabled"),
    ("Voice EQ", "multiBandEq.enabled"),
    ("De-esser", "deEsser.enabled"),
    ("De-popper", "dePopper.enabled"),
    ("Limiter", "hardLimiter.enabled"),
];

/// A preset's settings with fine-tuning (`path` → value) applied on top.
pub fn merged(settings: &Value, overrides: &std::collections::BTreeMap<String, Value>) -> Value {
    let mut s = settings.clone();
    for (path, value) in overrides {
        let mut at = &mut s;
        let mut parts = path.split('.').peekable();
        while let Some(k) = parts.next() {
            if !at.is_object() {
                *at = Value::Object(Default::default());
            }
            let obj = at.as_object_mut().unwrap();
            if parts.peek().is_none() {
                obj.insert(k.to_string(), value.clone());
                break;
            }
            at = obj
                .entry(k)
                .or_insert_with(|| Value::Object(Default::default()));
        }
    }
    s
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
        .join("glowden-voice-test")
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
    /// Latest (input, output) peak in dBFS from each running host, and when it came.
    levels: HashMap<String, Levels>,
}

type Levels = std::sync::Arc<std::sync::Mutex<Option<(f32, f32, std::time::Instant)>>>;

impl Voice {
    /// Runs the virtual microphone with `settings`: live update when it already runs (no
    /// interruption for apps), otherwise a fresh start.
    pub fn apply(&mut self, id: &str, def: &VoiceDef, args: &[String]) {
        if let Some(child) = self.running.get_mut(id)
            && matches!(child.try_wait(), Ok(None))
            && let Some(stdin) = child.stdin.as_mut()
        {
            use std::io::Write;
            let line = args.join(" ") + "\n";
            if stdin
                .write_all(line.as_bytes())
                .and_then(|_| stdin.flush())
                .is_ok()
            {
                return;
            }
        }
        self.start(id, def, args);
    }

    pub fn start(&mut self, id: &str, def: &VoiceDef, args: &[String]) {
        self.stop(id);
        match Self::spawn(def, args) {
            Ok(mut child) => {
                self.errors.remove(id);
                let levels = Levels::default();
                if let Some(out) = child.stdout.take() {
                    let into = levels.clone();
                    std::thread::spawn(move || {
                        use std::io::BufRead;
                        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                            let mut v = line.split(' ').filter_map(|x| x.parse::<f32>().ok());
                            if let (Some(i), Some(o)) = (v.next(), v.next()) {
                                *into.lock().unwrap() = Some((i, o, std::time::Instant::now()));
                            }
                        }
                    });
                }
                self.levels.insert(id.to_string(), levels);
                self.running.insert(id.to_string(), child);
            }
            Err(e) => {
                eprintln!("{id}: Blue VO!CE: {e}");
                self.errors.insert(id.to_string(), e);
            }
        }
    }

    fn spawn(def: &VoiceDef, args: &[String]) -> Result<Child, String> {
        let dll = engine().ok_or("the engine isn't imported yet (glowdenctl import-ghub)")?;
        let source = source_node(&def.source)
            .ok_or_else(|| format!("no PipeWire source matching \"{}\"", def.source))?;
        Command::new(binary())
            .arg(dll)
            .args(["--target", &source, "--name", "glowden-voice-mic"])
            .args(["--description", &def.description])
            .args(args)
            // Later settings arrive on stdin, applied without restarting; levels come back on stdout.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start glowden-voice: {e}"))
    }

    /// Plays the test recording as recorded (`None`) or with these host arguments.
    pub fn play_test(&mut self, args: Option<Vec<String>>) -> Result<(), String> {
        self.stop_test();
        let raw = test_dir().join("raw.wav");
        if !raw.exists() {
            return Err("record a test first".into());
        }
        let file = match args {
            None => raw,
            Some(s) => {
                let out = test_dir().join("processed.wav");
                let dll = engine().ok_or("the engine isn't imported yet")?;
                let status = Command::new(binary())
                    .arg(dll)
                    .args(s)
                    .arg("--process")
                    .args([&raw, &out])
                    .stdin(Stdio::null())
                    .status()
                    .map_err(|e| format!("cannot start glowden-voice: {e}"))?;
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

    /// The current input and output peaks (dBFS), while the host reports them.
    pub fn levels(&self, id: &str) -> Option<(f32, f32)> {
        let (i, o, at) = (*self.levels.get(id)?.lock().unwrap())?;
        (at.elapsed().as_millis() < 1000).then_some((i, o))
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
        if std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim() == "glowden-voice")
        {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

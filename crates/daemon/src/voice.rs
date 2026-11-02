//! Blue VO!CE for devices whose definition has a `[voice]` section. The processing runs in
//! the definition's plugin (a separate program from its device source; JSON lines on
//! stdin/stdout, see glowden-devices/plugins/blue-voice). This side keeps the settings,
//! picks the microphone, makes the processed one the default input and runs the preset test.
//! The engine DLL and the presets come from the user's G HUB folders (see assets.rs).

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::drivers::VoiceDef;

pub fn engine() -> Option<PathBuf> {
    crate::assets::current().engine.clone()
}

/// (name, settings) of each VO!CE preset in G HUB's settings, in G HUB's order.
pub fn presets() -> Vec<(String, Value)> {
    crate::assets::ghub_cards("BLUE_VOICE_2_SETTINGS")
        .into_iter()
        .filter_map(|c| {
            Some((
                c["name"].as_str()?.to_string(),
                c["blueVoice2Settings"]["settings"].clone(),
            ))
        })
        .collect()
}

/// Voice effect presets imported from G HUB: (name, settings), "Off" first.
pub fn effect_presets() -> Vec<(String, Value)> {
    // Named "NN_Name" in G HUB, NN giving its order; "00_FX_OFF" is shown as "Off".
    let mut cards: Vec<(String, Value)> = crate::assets::ghub_cards("AUDIO_EFFECTS_SETTINGS")
        .into_iter()
        .filter_map(|c| {
            let e = &c["audioEffectsSettings"];
            Some((
                e["description"]["name"].as_str()?.to_string(),
                e["settings"].clone(),
            ))
        })
        .collect();
    cards.sort_by(|a, b| a.0.cmp(&b.0));
    cards
        .into_iter()
        .map(|(name, s)| {
            let shown = name.split_once('_').map_or(name.as_str(), |(_, n)| n);
            let shown = if shown == "FX_OFF" { "Off" } else { shown };
            (shown.to_string(), s)
        })
        .collect()
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
            if pactl(&["set-default-source", NODE]).is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("Blue VO!CE: couldn't make the virtual microphone the default input");
    });
}

/// Hands the default input back to the real microphone if VO!CE had it.
pub fn release_default(def: &VoiceDef) {
    if pactl(&["get-default-source"]).as_deref() == Some(NODE)
        && let Some(source) = source_node(&def.source)
    {
        pactl(&["set-default-source", &source]);
    }
}

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

/// Node name of the processed microphone.
pub const NODE: &str = "glowden-voice-mic";

type Levels = Arc<Mutex<Option<(f32, f32, Instant)>>>;

/// A running plugin process and the JSON-lines conversation with it.
struct Plugin {
    child: Child,
    stdin: ChildStdin,
    next: u64,
    replies: Receiver<Value>,
    levels: Levels,
}

impl Plugin {
    fn spawn(binary: &Path) -> Result<Plugin, String> {
        let mut child = Command::new(binary)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start the plugin {}: {e}", binary.display()))?;
        let stdin = child.stdin.take().ok_or("plugin has no input")?;
        let stdout = child.stdout.take().ok_or("plugin has no output")?;
        let (tx, replies): (Sender<Value>, _) = channel();
        let levels = Levels::default();
        let into = levels.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
            {
                let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if msg["event"] == "levels" {
                    if let (Some(i), Some(o)) = (msg["input"].as_f64(), msg["output"].as_f64()) {
                        *into.lock().unwrap() = Some((i as f32, o as f32, Instant::now()));
                    }
                } else if !msg["id"].is_null() && tx.send(msg).is_err() {
                    break;
                }
            }
        });
        Ok(Plugin {
            child,
            stdin,
            next: 0,
            replies,
            levels,
        })
    }

    fn call(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let msg = json!({ "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{msg}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("the plugin stopped: {e}"))?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let reply = self
                .replies
                .recv_timeout(left)
                .map_err(|_| format!("the plugin didn't answer {method}"))?;
            if reply["id"] != id {
                continue;
            }
            return match reply["error"].as_str() {
                Some(e) => Err(e.to_string()),
                None => Ok(reply["result"].clone()),
            };
        }
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        // Closing its input stops the plugin and its host.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What a plugin offers the UI (controls, switches, effect modes) and its preset summaries.
pub fn describe(binary: &Path, presets: &[(String, Value)]) -> Result<(Value, Value), String> {
    static CACHE: Mutex<Option<(PathBuf, Value, Value)>> = Mutex::new(None);
    let names: Vec<Value> = presets
        .iter()
        .map(|(n, s)| json!({ "name": n, "settings": s }))
        .collect();
    let mut cache = CACHE.lock().unwrap();
    if let Some((b, d, s)) = cache.as_ref()
        && b == binary
        && s.as_object().is_some_and(|m| m.len() == names.len())
    {
        return Ok((d.clone(), s.clone()));
    }
    let mut p = Plugin::spawn(binary)?;
    let d = p.call("describe", Value::Null, Duration::from_secs(5))?;
    let s = p.call(
        "summarize",
        json!({ "presets": names }),
        Duration::from_secs(5),
    )?;
    *cache = Some((binary.to_path_buf(), d.clone(), s.clone()));
    Ok((d, s))
}

#[derive(Default)]
pub struct Voice {
    running: HashMap<String, Plugin>,
    playing: Option<Child>,
    /// Why the last start for a device failed, for the UI.
    errors: HashMap<String, String>,
}

impl Voice {
    /// Runs the processed microphone with these settings: a live update when it already
    /// runs (no interruption for apps), otherwise a fresh start.
    pub fn apply(
        &mut self,
        id: &str,
        def: &VoiceDef,
        plugin: Result<PathBuf, String>,
        voice: Option<&Value>,
        effects: Option<&Value>,
    ) {
        let params = json!({ "voice": voice, "effects": effects });
        if let Some(p) = self.running.get_mut(id)
            && p.alive()
            && p.call("apply", params.clone(), Duration::from_secs(5))
                .is_ok()
        {
            return;
        }
        self.stop(id);
        let started = (|| {
            let binary = plugin?;
            let engine = engine().ok_or(
                "the Blue VO!CE engine wasn't found in your G HUB folders (Settings → G HUB files)",
            )?;
            let source = source_node(&def.source)
                .ok_or_else(|| format!("no PipeWire source matching \"{}\"", def.source))?;
            let mut p = Plugin::spawn(&binary)?;
            let mut start = params;
            start["engine"] = json!(engine);
            start["source"] = json!(source);
            start["name"] = json!(NODE);
            start["description"] = json!(def.description);
            p.call("start", start, Duration::from_secs(10))?;
            Ok::<Plugin, String>(p)
        })();
        match started {
            Ok(p) => {
                self.errors.remove(id);
                self.running.insert(id.to_string(), p);
            }
            Err(e) => {
                eprintln!("{id}: Blue VO!CE: {e}");
                self.errors.insert(id.to_string(), e);
            }
        }
    }

    /// Plays the test recording as recorded (`None`) or processed by the plugin.
    pub fn play_test(
        &mut self,
        processed: Option<(PathBuf, Option<Value>, Option<Value>)>,
    ) -> Result<(), String> {
        self.stop_test();
        let raw = test_dir().join("raw.wav");
        if !raw.exists() {
            return Err("record a test first".into());
        }
        let file = match processed {
            None => raw,
            Some((binary, voice, effects)) => {
                let out = test_dir().join("processed.wav");
                let engine =
                    engine().ok_or("the Blue VO!CE engine wasn't found in your G HUB folders")?;
                let mut p = Plugin::spawn(&binary)?;
                p.call(
                    "process",
                    json!({ "engine": engine, "in": raw, "out": out, "voice": voice, "effects": effects }),
                    Duration::from_secs(120),
                )?;
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

    /// The current input and output peaks (dBFS), while the plugin reports them.
    pub fn levels(&self, id: &str) -> Option<(f32, f32)> {
        let (i, o, at) = (*self.running.get(id)?.levels.lock().unwrap())?;
        (at.elapsed().as_millis() < 1000).then_some((i, o))
    }

    pub fn stop_test(&mut self) {
        if let Some(mut child) = self.playing.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn stop(&mut self, id: &str) {
        self.running.remove(id);
    }

    /// Whether the processed microphone is up; a crashed plugin is noticed and reported.
    pub fn status(&mut self, id: &str) -> (bool, Option<String>) {
        if let Some(p) = self.running.get_mut(id)
            && let Ok(Some(code)) = p.child.try_wait()
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

/// Hosts left over from older versions, which ran the engine without a plugin.
pub fn stop_stale() {
    for e in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if std::fs::read_to_string(e.path().join("comm"))
            .is_ok_and(|c| matches!(c.trim(), "glowden-voice" | "lhub-voice"))
        {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

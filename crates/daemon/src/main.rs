//! lhubd: owns the devices, keeps their configuration applied, and serves the UI over a
//! Unix socket with one JSON request/response per line.

mod app;
mod battery;
mod config;
mod drivers;
mod focus;
mod keys;
mod uinput;
mod view;
mod voice;
mod worker;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use config::{Action, AppSettings, Config, DeviceConfig, Lighting, VoiceConfig};
use worker::{Cmd, Handle};

struct State {
    config: Config,
    drivers: drivers::Registry,
    workers: HashMap<String, Handle>,
    scanned: bool,
    voice: voice::Voice,
    /// Profile in effect (None = default profile).
    active: Option<String>,
    /// Window classes focused lately, newest first, for linking apps to profiles.
    recent: Vec<String>,
    focused: Option<String>,
    /// Devices showing the low-battery effect.
    low_battery: std::collections::HashSet<String>,
}

impl State {
    fn effective(&self, id: &str) -> DeviceConfig {
        let mut cfg = self.config.effective(id, self.active.as_deref());
        if self.low_battery.contains(id) && cfg.lighting.is_some() {
            cfg.lighting = Some(Lighting::Breathing {
                color: "#ff1010".into(),
                speed: 40,
                intensity: 100,
            });
            cfg.power = Some(true);
        }
        cfg
    }

    /// Goes back to the default DPI level, where the current level is kept (the active
    /// profile's override, else the device's own config).
    fn reset_dpi(&mut self, id: &str) {
        let Some(default) = self.effective(id).dpi_default else {
            return;
        };
        let active = self.active.clone();
        let in_profile = active.as_ref().and_then(|name| {
            self.config
                .profiles
                .iter_mut()
                .find(|p| &p.name == name)?
                .devices
                .get_mut(id)
                .filter(|d| d.dpi_level.is_some())
        });
        match in_profile {
            Some(d) => d.dpi_level = Some(default),
            None => {
                self.config
                    .devices
                    .entry(id.to_string())
                    .or_default()
                    .dpi_level = Some(default)
            }
        }
        self.config.save();
    }

    /// Re-applies every connected device after the active profile changed.
    fn apply_all(&self) {
        for (id, h) in &self.workers {
            let _ = h.tx.send(Cmd::Apply(self.effective(id)));
        }
    }

    /// Which profile should be active now: the focused app's when switching automatically,
    /// else the one picked by hand.
    fn wanted_profile(&self) -> Option<String> {
        if !self.config.profile_auto {
            return self
                .config
                .active_profile
                .clone()
                .filter(|n| self.config.profiles.iter().any(|p| &p.name == n));
        }
        let class = self.focused.as_deref()?.to_lowercase();
        self.config
            .profiles
            .iter()
            .find(|p| p.apps.iter().any(|a| a.to_lowercase() == class))
            .map(|p| p.name.clone())
    }

    fn switch_profile(&mut self) {
        let wanted = self.wanted_profile();
        if wanted != self.active {
            eprintln!("profile: {}", wanted.as_deref().unwrap_or("default"));
            self.active = wanted;
            let ids: Vec<String> = self.workers.keys().cloned().collect();
            for id in ids {
                self.reset_dpi(&id);
            }
            self.apply_all();
        }
    }
}

type Shared = Arc<(Mutex<State>, Condvar)>;

pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join("lhub.sock")
}

/// Holds an exclusive lock for the life of the process, so two services started at the
/// same moment (e.g. by the app and by systemd) can't both claim the socket.
fn single_instance() -> Option<std::fs::File> {
    use std::os::fd::AsRawFd;
    let lock = socket_path().with_extension("lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .ok()?;
    let ok = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    ok.then_some(file)
}

fn main() {
    let path = socket_path();
    let Some(_lock) = single_instance() else {
        eprintln!("lhubd already running ({})", path.display());
        return;
    };
    // Only the instance that owns the lock may clean up, or a second start would kill the
    // running service's hosts.
    voice::stop_stale();
    let (focus_tx, focus_rx) = mpsc::channel::<focus::Focus>();
    focus::spawn(focus_tx);
    let _ = std::fs::remove_file(&path);
    let listener =
        UnixListener::bind(&path).unwrap_or_else(|e| panic!("bind {}: {e}", path.display()));
    eprintln!("lhubd listening on {}", path.display());

    let shared: Shared = Arc::new((
        Mutex::new(State {
            config: Config::load(),
            drivers: drivers::Registry::load(),
            workers: HashMap::new(),
            scanned: false,
            voice: voice::Voice::default(),
            active: None,
            recent: Vec::new(),
            focused: None,
            low_battery: Default::default(),
        }),
        Condvar::new(),
    ));

    let manager = shared.clone();
    std::thread::spawn(move || manage(manager));
    shared.0.lock().unwrap().switch_profile();
    let watcher = shared.clone();
    std::thread::spawn(move || {
        let mut last = app::lock_keys();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let now = app::lock_keys();
            if now != last && watcher.0.lock().unwrap().config.app.lock_notifications {
                if now.0 != last.0 {
                    app::notify_lock("Caps Lock", now.0);
                }
                if now.1 != last.1 {
                    app::notify_lock("Num Lock", now.1);
                }
            }
            last = now;
        }
    });
    let focuser = shared.clone();
    std::thread::spawn(move || {
        for (class, _caption) in focus_rx {
            if class.is_empty() {
                continue;
            }
            let mut st = focuser.0.lock().unwrap();
            st.recent.retain(|c| c != &class);
            st.recent.insert(0, class.clone());
            st.recent.truncate(15);
            st.focused = Some(class);
            st.switch_profile();
        }
    });

    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || serve(stream, shared));
    }
}

/// Picks up new and reconnected devices and hands each its saved configuration.
fn manage(shared: Shared) {
    // Nodes that didn't answer, and when; they're only re-probed every 30 s so a switched-off
    // headset doesn't slow down noticing everything else.
    let mut silent: HashMap<String, std::time::Instant> = HashMap::new();
    let mut warned = std::collections::HashSet::new();
    let mut locked = false;
    loop {
        {
            let mut st = shared.0.lock().unwrap();
            st.workers.retain(|id, h| {
                let alive = h.alive.load(Ordering::Relaxed);
                if !alive {
                    eprintln!("{id}: gone");
                }
                alive
            });
            let gone: Vec<String> = st
                .config
                .devices
                .keys()
                .filter(|id| !st.workers.contains_key(*id))
                .cloned()
                .collect();
            for id in gone {
                st.voice.stop(&id);
            }
            // DPI levels picked with the mouse's buttons are remembered like UI changes.
            let picked: Vec<(String, usize)> = st
                .workers
                .iter()
                .filter_map(|(id, h)| Some((id.clone(), h.dpi_level.lock().unwrap().take()?)))
                .collect();
            if !picked.is_empty() {
                for (id, level) in picked {
                    st.config.devices.entry(id).or_default().dpi_level = Some(level);
                }
                st.config.save();
            }
            // Low-battery lighting: on at 15 % (not charging), off once charging or above 20 %.
            let changes: Vec<(String, bool)> = st
                .workers
                .iter()
                .filter_map(|(id, h)| {
                    let b = h.view.lock().unwrap().battery.clone()?;
                    let low = st.low_battery.contains(id);
                    let now = st.config.app.low_battery_lighting
                        && if low {
                            !b.charging && b.percent <= 20
                        } else {
                            !b.charging && b.percent <= 15
                        };
                    (now != low).then(|| (id.clone(), now))
                })
                .collect();
            for (id, low) in changes {
                if low {
                    st.low_battery.insert(id.clone());
                } else {
                    st.low_battery.remove(&id);
                }
                let cfg = st.effective(&id);
                if let Some(h) = st.workers.get(&id) {
                    let _ = h.tx.send(Cmd::Apply(cfg));
                }
            }
            if st.config.app.low_battery_alert {
                for (id, h) in &st.workers {
                    let v = h.view.lock().unwrap();
                    let Some(b) = &v.battery else { continue };
                    // Once per discharge: re-armed after charging past 20 %.
                    if b.percent <= 15 && !b.charging {
                        if warned.insert(id.clone()) {
                            app::notify(
                                &format!("{} battery low", v.name),
                                &format!("{} % left", b.percent),
                            );
                        }
                    } else if b.charging || b.percent > 20 {
                        warned.remove(id);
                    }
                }
            }
        }
        {
            let st = shared.0.lock().unwrap();
            let now = st.config.app.lighting_off_when_locked && app::session_locked();
            if now != locked {
                locked = now;
                eprintln!(
                    "session {}: lighting {}",
                    if locked { "locked" } else { "unlocked" },
                    if locked { "off" } else { "restored" }
                );
                // Not saved: the configured lighting comes back on unlock.
                for (id, h) in &st.workers {
                    let mut cfg = st.effective(id);
                    if cfg.lighting.is_none() {
                        continue;
                    }
                    if locked {
                        cfg.power = Some(false);
                    } else if cfg.brightness.is_none() {
                        // Devices without an "off" effect were dimmed to 0 while locked.
                        cfg.brightness = Some(100);
                    }
                    let _ = h.tx.send(Cmd::Apply(cfg));
                }
            }
        }
        let known: Vec<String> = shared.0.lock().unwrap().workers.keys().cloned().collect();
        silent.retain(|_, at| at.elapsed() < Duration::from_secs(30));
        let skip = |id: &str| known.iter().any(|k| k == id) || silent.contains_key(id);
        let quiet = hidpp::discover::scan_each(&skip, &mut |f| {
            let mut st = shared.0.lock().unwrap();
            st.reset_dpi(&f.id);
            let cfg = st.effective(&f.id);
            eprintln!("{}: {} connected, applying {:?}", f.id, f.name, cfg);
            let id = f.id.clone();
            let driver = st.drivers.for_product(f.device.info.product).cloned();
            let voice = cfg.voice.clone().filter(|v| v.active());
            st.workers
                .insert(id.clone(), worker::spawn(f, cfg, driver.clone()));
            if let (Some(v), Some(def)) = (voice, driver.and_then(|d| d.voice)) {
                start_voice(&mut st, &id, &def, &v);
                if v.default_mic {
                    voice::make_default();
                }
            }
            drop(st);
            shared.1.notify_all();
        });
        let now = std::time::Instant::now();
        silent.extend(quiet.into_iter().map(|id| (id, now)));
        shared.0.lock().unwrap().scanned = true;
        shared.1.notify_all();
        std::thread::sleep(Duration::from_secs(3));
    }
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    List,
    GetConfig {
        id: String,
    },
    SetLighting {
        id: String,
        lighting: Lighting,
    },
    SetBrightness {
        id: String,
        percent: u8,
    },
    Release {
        id: String,
    },
    GetGain {
        id: String,
    },
    SetGain {
        id: String,
        gain: u8,
    },
    SetMuted {
        id: String,
        muted: bool,
    },
    GetMuted {
        id: String,
    },
    DeviceImage {
        id: String,
    },
    DeviceLayout {
        id: String,
    },
    LightingState {
        id: String,
    },
    SetPower {
        id: String,
        on: bool,
    },
    SyncLighting {
        from: String,
    },
    MouseInfo {
        id: String,
    },
    SetDpi {
        id: String,
        levels: Vec<u16>,
        level: usize,
        #[serde(default)]
        default: Option<usize>,
        #[serde(default)]
        shift: Option<u16>,
    },
    SetReportInterval {
        id: String,
        ms: u8,
    },
    VoiceState {
        id: String,
    },
    VoiceTestRecord {
        id: String,
        seconds: u64,
    },
    /// Plays the test recording through a VO!CE preset and/or a voice effect (neither: as
    /// recorded).
    VoiceTestPlay {
        preset: Option<String>,
        #[serde(default)]
        effect: Option<String>,
    },
    VoiceTestStop,
    VoiceLevels {
        id: String,
    },
    LightingPresets,
    SaveLightingPreset {
        name: String,
        lighting: Lighting,
    },
    DeleteLightingPreset {
        name: String,
    },
    ExportLightingPresets,
    ImportLightingPresets {
        path: String,
    },
    Backups,
    SaveBackup,
    RestoreBackup {
        name: String,
    },
    DeleteBackup {
        name: String,
    },
    Profiles,
    SetActiveProfile {
        name: Option<String>,
    },
    /// Creates or updates a profile; `old` renames.
    SaveProfile {
        name: String,
        apps: Vec<String>,
        old: Option<String>,
    },
    DeleteProfile {
        name: String,
    },
    SetProfileAuto {
        on: bool,
    },
    ExportProfile {
        name: String,
    },
    ImportProfile {
        path: String,
    },
    Buttons {
        id: String,
    },
    SetAssignment {
        id: String,
        button: u8,
        action: Option<Action>,
    },
    AppSettings,
    SetAppSettings {
        settings: AppSettings,
    },
    SetAutostart {
        on: bool,
    },
    ImportGhub {
        path: String,
    },
    CreateDriver {
        id: String,
    },
    SetVoice {
        id: String,
        enabled: bool,
        preset: Option<String>,
        #[serde(default = "yes")]
        default_mic: bool,
    },
    /// Fine-tuning on top of the current preset (an empty map resets to the preset).
    SetVoiceOverrides {
        id: String,
        overrides: std::collections::BTreeMap<String, Value>,
    },
    SetVoiceEffect {
        id: String,
        effect: Option<String>,
    },
    /// Fine-tuning on top of the current effect preset (an empty map resets it).
    SetVoiceEffectOverrides {
        id: String,
        overrides: std::collections::BTreeMap<String, Value>,
    },
}

fn serve(stream: UnixStream, shared: Shared) {
    let mut out = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => match handle(req, &shared) {
                Ok(data) => json!({ "ok": true, "data": data }),
                Err(e) => json!({ "ok": false, "error": e }),
            },
            Err(e) => json!({ "ok": false, "error": format!("bad request: {e}") }),
        };
        if writeln!(out, "{response}").is_err() {
            return;
        }
    }
}

fn handle(req: Request, shared: &Shared) -> Result<Value, String> {
    let (lock, cvar) = &**shared;
    match req {
        Request::List => {
            let st = cvar
                .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(8), |s| !s.scanned)
                .unwrap()
                .0;
            let mut views: Vec<_> = st
                .workers
                .values()
                .map(|h| h.view.lock().unwrap().clone())
                .collect();
            for v in views.iter_mut() {
                let cfg = st.config.devices.get(&v.id).cloned().unwrap_or_default();
                if let (Some(b), Some(model)) = (v.battery.as_mut(), v.model.as_deref()) {
                    b.hours = battery::hours_left(model, b.percent, &cfg);
                }
            }
            views.sort_by(|a, b| a.kind.cmp(b.kind).then(a.name.cmp(&b.name)));
            Ok(json!(views))
        }
        Request::GetConfig { id } => {
            let st = lock.lock().unwrap();
            Ok(json!(st.effective(&id)))
        }
        Request::SetLighting { id, lighting } => {
            update(shared, &id, |c| c.lighting = Some(lighting))
        }
        Request::SetBrightness { id, percent } => {
            update(shared, &id, |c| c.brightness = Some(percent.min(100)))
        }
        Request::Release { id } => {
            {
                let mut st = lock.lock().unwrap();
                if let Some(c) = st.config.devices.get_mut(&id) {
                    c.lighting = None;
                }
                st.config.save();
            }
            ask(shared, &id, Cmd::Release)
        }
        Request::GetGain { id } => ask(shared, &id, Cmd::GetGain),
        Request::SetGain { id, gain } => ask(shared, &id, |tx| Cmd::SetGain(gain, tx)),
        Request::SetMuted { id, muted } => ask(shared, &id, |tx| Cmd::SetMuted(muted, tx)),
        Request::GetMuted { id } => ask(shared, &id, Cmd::GetMuted),
        Request::SetPower { id, on } => update(shared, &id, |c| c.power = Some(on)),
        Request::SyncLighting { from } => sync_lighting(shared, &from),
        Request::VoiceState { id } => Ok(voice_state(&mut lock.lock().unwrap(), &id)),
        Request::VoiceTestRecord { id, seconds } => {
            // Recorded without holding the state lock; the mic's PipeWire node does the work.
            let def =
                voice_def(&lock.lock().unwrap(), &id).ok_or("this device has no Blue VO!CE")?;
            voice::record_test(&def, seconds).map(|_| Value::Null)
        }
        Request::VoiceTestPlay { preset, effect } => {
            let find = |list: Vec<(String, Value)>,
                        name: Option<String>|
             -> Result<Option<Value>, String> {
                match name {
                    Some(name) => list
                        .into_iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, s)| Some(s))
                        .ok_or_else(|| format!("no preset called {name}")),
                    None => Ok(None),
                }
            };
            let settings = find(voice::presets(), preset)?;
            let effects = find(voice::effect_presets(), effect)?;
            let args = (settings.is_some() || effects.is_some())
                .then(|| voice::host_args(settings.as_ref(), effects.as_ref()));
            let mut st = lock.lock().unwrap();
            st.voice.play_test(args).map(|_| Value::Null)
        }
        Request::VoiceLevels { id } => Ok(match lock.lock().unwrap().voice.levels(&id) {
            Some((input, output)) => json!({ "input": input, "output": output }),
            None => Value::Null,
        }),
        Request::VoiceTestStop => {
            lock.lock().unwrap().voice.stop_test();
            Ok(Value::Null)
        }
        Request::LightingPresets => Ok(json!(lock.lock().unwrap().config.lighting_presets)),
        Request::SaveLightingPreset { name, lighting } => {
            let mut st = lock.lock().unwrap();
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err("give the preset a name".into());
            }
            let presets = &mut st.config.lighting_presets;
            match presets.iter_mut().find(|p| p.name == name) {
                Some(p) => p.lighting = lighting,
                None => presets.push(config::LightingPreset { name, lighting }),
            }
            st.config.save();
            Ok(json!(st.config.lighting_presets))
        }
        Request::DeleteLightingPreset { name } => {
            let mut st = lock.lock().unwrap();
            st.config.lighting_presets.retain(|p| p.name != name);
            st.config.save();
            Ok(json!(st.config.lighting_presets))
        }
        Request::ExportLightingPresets => {
            let st = lock.lock().unwrap();
            let file = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join("lhub-lighting-presets.json");
            std::fs::write(
                &file,
                serde_json::to_vec_pretty(&st.config.lighting_presets).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            Ok(json!(file.display().to_string()))
        }
        Request::ImportLightingPresets { path } => {
            let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
            let list: Vec<config::LightingPreset> =
                serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
            let mut st = lock.lock().unwrap();
            for mut p in list {
                while st.config.lighting_presets.iter().any(|q| q.name == p.name) {
                    p.name.push_str(" (imported)");
                }
                st.config.lighting_presets.push(p);
            }
            st.config.save();
            Ok(json!(st.config.lighting_presets))
        }
        Request::Backups => Ok(backups()),
        Request::SaveBackup => {
            let st = lock.lock().unwrap();
            st.config.save();
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            std::fs::create_dir_all(backup_dir()).map_err(|e| e.to_string())?;
            std::fs::copy(config::path(), backup_dir().join(format!("{secs}.json")))
                .map_err(|e| e.to_string())?;
            Ok(backups())
        }
        Request::RestoreBackup { name } => {
            let file = backup_dir().join(format!("{}.json", name.trim_end_matches(".json")));
            let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let restored: Config = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let mut st = lock.lock().unwrap();
            st.config = restored;
            st.config.save();
            st.active = None;
            st.switch_profile();
            st.apply_all();
            Ok(backups())
        }
        Request::DeleteBackup { name } => {
            let file = backup_dir().join(format!("{}.json", name.trim_end_matches(".json")));
            std::fs::remove_file(file).map_err(|e| e.to_string())?;
            Ok(backups())
        }
        Request::Profiles => Ok(profiles(&lock.lock().unwrap())),
        Request::SetActiveProfile { name } => {
            let mut st = lock.lock().unwrap();
            st.config.active_profile = name.clone();
            st.config.save();
            // Picked by hand: applies now, even while switching automatically.
            if st.active != name {
                st.active = name;
                st.apply_all();
            }
            Ok(profiles(&st))
        }
        Request::SaveProfile { name, apps, old } => {
            let mut st = lock.lock().unwrap();
            let name = name.trim().to_string();
            if name.is_empty() || name.eq_ignore_ascii_case("default") {
                return Err("pick another name".into());
            }
            let key = old.unwrap_or_else(|| name.clone());
            if key != name && st.config.profiles.iter().any(|p| p.name == name) {
                return Err(format!("a profile called {name} exists"));
            }
            match st.config.profiles.iter_mut().find(|p| p.name == key) {
                Some(p) => {
                    p.name = name.clone();
                    p.apps = apps;
                }
                None => st.config.profiles.push(config::Profile {
                    name: name.clone(),
                    apps,
                    devices: Default::default(),
                }),
            }
            if st.active.as_deref() == Some(key.as_str()) {
                st.active = Some(name.clone());
            }
            if st.config.active_profile.as_deref() == Some(key.as_str()) {
                st.config.active_profile = Some(name);
            }
            st.config.save();
            st.switch_profile();
            Ok(profiles(&st))
        }
        Request::DeleteProfile { name } => {
            let mut st = lock.lock().unwrap();
            st.config.profiles.retain(|p| p.name != name);
            if st.config.active_profile.as_deref() == Some(name.as_str()) {
                st.config.active_profile = None;
            }
            if st.active.as_deref() == Some(name.as_str()) {
                st.active = None;
                st.apply_all();
            }
            st.config.save();
            Ok(profiles(&st))
        }
        Request::SetProfileAuto { on } => {
            let mut st = lock.lock().unwrap();
            st.config.profile_auto = on;
            st.config.save();
            st.switch_profile();
            Ok(profiles(&st))
        }
        Request::ExportProfile { name } => {
            let st = lock.lock().unwrap();
            let p = st
                .config
                .profiles
                .iter()
                .find(|p| p.name == name)
                .ok_or("no such profile")?;
            let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
            let file = dir.join(format!(
                "lhub-profile-{}.json",
                name.replace(['/', ' '], "-")
            ));
            std::fs::write(&file, serde_json::to_vec_pretty(p).unwrap())
                .map_err(|e| e.to_string())?;
            Ok(json!(file.display().to_string()))
        }
        Request::ImportProfile { path } => {
            let mut st = lock.lock().unwrap();
            let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
            let mut p: config::Profile =
                serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
            while st.config.profiles.iter().any(|q| q.name == p.name) {
                p.name.push_str(" (imported)");
            }
            st.config.profiles.push(p);
            st.config.save();
            Ok(profiles(&st))
        }
        Request::Buttons { id } => buttons(&lock.lock().unwrap(), &id),
        Request::SetAssignment { id, button, action } => update(shared, &id, |c| match action {
            Some(a) => {
                c.assignments.insert(button, a);
            }
            None => {
                c.assignments.remove(&button);
            }
        }),
        Request::AppSettings => Ok(app_settings(&lock.lock().unwrap())),
        Request::SetAppSettings { settings } => {
            let mut st = lock.lock().unwrap();
            let swap_changed = st.config.app.swap_mouse_buttons != settings.swap_mouse_buttons;
            st.config.app = settings;
            st.config.save();
            if swap_changed {
                st.apply_all();
            }
            Ok(app_settings(&st))
        }
        Request::SetAutostart { on } => app::set_autostart(on).map(|_| json!(app::autostart())),
        Request::ImportGhub { path } => app::import_ghub(&path).map(|t| json!(t)),
        Request::CreateDriver { id } => create_driver(shared, &id),
        Request::SetVoice {
            id,
            enabled,
            preset,
            default_mic,
        } => set_voice(
            shared,
            &id,
            VoiceConfig {
                enabled,
                preset,
                default_mic,
                ..Default::default()
            },
        ),
        Request::SetVoiceEffect { id, effect } => update_voice(shared, &id, |v| {
            if v.effect != effect {
                v.effect_overrides.clear();
            }
            v.effect = effect;
        }),
        Request::SetVoiceEffectOverrides { id, overrides } => {
            update_voice(shared, &id, |v| v.effect_overrides = overrides)
        }
        Request::SetVoiceOverrides { id, overrides } => {
            let mut st = lock.lock().unwrap();
            let def = voice_def(&st, &id).ok_or("this device has no Blue VO!CE")?;
            let cfg = {
                let v = st
                    .config
                    .devices
                    .entry(id.clone())
                    .or_default()
                    .voice
                    .get_or_insert_default();
                v.overrides = overrides;
                v.clone()
            };
            st.config.save();
            if cfg.active() {
                start_voice(&mut st, &id, &def, &cfg);
            }
            Ok(voice_state(&mut st, &id))
        }
        Request::MouseInfo { id } => ask(shared, &id, Cmd::MouseInfo),
        Request::DeviceLayout { id } => {
            // Cloned out so a first-time photo download doesn't hold the state lock.
            let v = {
                let st = lock.lock().unwrap();
                st.workers.get(&id).map(|h| h.view.lock().unwrap().clone())
            };
            Ok(json!(v.and_then(|v| {
                let model = v.model.as_deref()?;
                v.photo
                    .as_ref()
                    .and_then(|p| view::photo_layout(model, p))
                    .or_else(|| {
                        view::layout(
                            model,
                            v.hid_usage_zones,
                            v.first_view,
                            v.fallback_picture.as_deref(),
                            v.picture_clip.as_deref(),
                        )
                    })
            })))
        }
        Request::LightingState { id } => {
            let st = lock.lock().unwrap();
            let h = st
                .workers
                .get(&id)
                .ok_or_else(|| format!("device {id} is not connected"))?;
            Ok(json!(*h.lights.lock().unwrap()))
        }
        Request::DeviceImage { id } => {
            let st = lock.lock().unwrap();
            let model = st
                .workers
                .get(&id)
                .and_then(|h| h.view.lock().unwrap().model.clone());
            Ok(json!(model.as_deref().and_then(view::image)))
        }
        Request::SetDpi {
            id,
            levels,
            level,
            default,
            shift,
        } => {
            if levels.is_empty() || levels.len() > 5 || level >= levels.len() {
                return Err("need 1-5 DPI levels and a valid active level".into());
            }
            if default.is_some_and(|d| d >= levels.len()) {
                return Err("the default level must be one of the levels".into());
            }
            update(shared, &id, |c| {
                c.dpi_levels = levels;
                c.dpi_level = Some(level);
                c.dpi_default = default;
                c.dpi_shift = shift;
            })
        }
        Request::SetReportInterval { id, ms } => {
            update(shared, &id, |c| c.report_interval = Some(ms))
        }
    }
}

/// Changes a device's saved configuration and applies it if the device is connected.
fn update(
    shared: &Shared,
    id: &str,
    change: impl FnOnce(&mut DeviceConfig),
) -> Result<Value, String> {
    let mut st = shared.0.lock().unwrap();
    // While a profile is active, changes are saved into it (as G HUB edits the active profile).
    let active = st.active.clone();
    let target = match active
        .as_ref()
        .and_then(|name| st.config.profiles.iter_mut().find(|p| &p.name == name))
    {
        Some(p) => p.devices.entry(id.to_string()).or_default(),
        None => st.config.devices.entry(id.to_string()).or_default(),
    };
    change(target);
    st.config.save();
    let cfg = st.effective(id);
    if let Some(h) = st.workers.get(id) {
        let _ = h.tx.send(Cmd::Apply(cfg));
    }
    Ok(Value::Null)
}

/// Sends a command to a device's worker and waits for its answer.
fn ask(
    shared: &Shared,
    id: &str,
    make: impl FnOnce(worker::Reply) -> Cmd,
) -> Result<Value, String> {
    let (tx, rx) = mpsc::channel();
    {
        let st = shared.0.lock().unwrap();
        let h = st
            .workers
            .get(id)
            .ok_or_else(|| format!("device {id} is not connected"))?;
        h.tx.send(make(tx))
            .map_err(|_| format!("device {id} is not connected"))?;
    }
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| format!("device {id} did not respond"))?
}

/// Copies a device's lighting to every other connected device that can show it. Firmware
/// and freestyle effects are device-specific; live effects need per-key lighting.
/// Copies lighting to every device that can show it. Device-specific effects (firmware
/// effects, Freestyle) aren't synced, and devices running one keep it.
fn sync_lighting(shared: &Shared, from: &str) -> Result<Value, String> {
    let (lighting, brightness, targets) = {
        let st = shared.0.lock().unwrap();
        let src = st.effective(from);
        let lighting = src.lighting.ok_or("this device has no lighting set yet")?;
        if !lighting.is_syncable() {
            return Err("this effect is specific to this device and can't be synced".into());
        }
        let targets: Vec<(String, String)> = st
            .workers
            .iter()
            .filter(|(id, _)| id.as_str() != from)
            .filter(|(id, _)| {
                st.config
                    .devices
                    .get(id.as_str())
                    .and_then(|c| c.lighting.as_ref())
                    .is_none_or(Lighting::is_syncable)
            })
            .filter_map(|(id, h)| {
                let v = h.view.lock().unwrap();
                let shows = v.caps.contains(&"lighting")
                    && (!lighting.is_host_rendered() || v.caps.contains(&"per_key"))
                    && (!matches!(lighting, Lighting::Pulse { .. })
                        || v.caps.contains(&"reactive"));
                shows.then(|| (id.clone(), v.name.clone()))
            })
            .collect();
        (lighting, src.brightness, targets)
    };
    let mut synced = Vec::new();
    for (id, name) in targets {
        let l = lighting.clone();
        update(shared, &id, |c| {
            c.lighting = Some(l);
            c.power = Some(true);
            if brightness.is_some() {
                c.brightness = brightness;
            }
        })?;
        synced.push(name);
    }
    Ok(json!(synced))
}

fn voice_def(st: &State, id: &str) -> Option<drivers::VoiceDef> {
    let product = st.workers.get(id)?.view.lock().unwrap().product.clone();
    let product = u16::from_str_radix(product.rsplit(':').next()?, 16).ok()?;
    st.drivers.for_product(product)?.voice.clone()
}

/// Starts the virtual microphone with the configured preset (the first one by default).
/// The chosen preset (the first one by default) with the fine-tuning applied.
fn voice_settings(cfg: &VoiceConfig) -> Value {
    let presets = voice::presets();
    let chosen = cfg
        .preset
        .as_ref()
        .and_then(|name| presets.iter().find(|(n, _)| n == name))
        .or(presets.first());
    let settings = chosen.map(|(_, s)| s.clone()).unwrap_or(Value::Null);
    voice::merged(&settings, &cfg.overrides)
}

/// The chosen voice effect with its fine-tuning, when effects are on.
fn effect_settings(cfg: &VoiceConfig) -> Option<Value> {
    let name = cfg.effect.as_ref().filter(|_| cfg.effect_on())?;
    let (_, s) = voice::effect_presets()
        .into_iter()
        .find(|(n, _)| n == name)?;
    Some(voice::merged(&s, &cfg.effect_overrides))
}

fn start_voice(st: &mut State, id: &str, def: &drivers::VoiceDef, cfg: &VoiceConfig) {
    let settings = cfg.enabled.then(|| voice_settings(cfg));
    let args = voice::host_args(settings.as_ref(), effect_settings(cfg).as_ref());
    st.voice.apply(id, def, &args);
}

/// Changes the voice effect part of a device's VO!CE config and applies it.
fn update_voice(
    shared: &Shared,
    id: &str,
    change: impl FnOnce(&mut VoiceConfig),
) -> Result<Value, String> {
    let mut st = shared.0.lock().unwrap();
    let def = voice_def(&st, id).ok_or("this device has no Blue VO!CE")?;
    let cfg = {
        let v = st
            .config
            .devices
            .entry(id.to_string())
            .or_default()
            .voice
            .get_or_insert_with(|| VoiceConfig {
                default_mic: true,
                ..Default::default()
            });
        change(v);
        v.clone()
    };
    st.config.save();
    if cfg.active() {
        start_voice(&mut st, id, &def, &cfg);
        if cfg.default_mic {
            voice::make_default();
        }
    } else {
        st.voice.stop(id);
        voice::release_default(&def);
    }
    Ok(voice_state(&mut st, id))
}

fn voice_state(st: &mut State, id: &str) -> Value {
    let cfg = st
        .config
        .devices
        .get(id)
        .and_then(|c| c.voice.clone())
        .unwrap_or_default();
    let (running, error) = st.voice.status(id);
    let all = voice::presets();
    let presets: Vec<String> = all.iter().map(|(n, _)| n.clone()).collect();
    let details: serde_json::Map<String, Value> = all
        .iter()
        .map(|(n, s)| (n.clone(), voice::summary(s)))
        .collect();
    json!({
        "details": details,
        "has_test": voice::has_test(),
        "supported": voice_def(st, id).is_some(),
        "engine": voice::engine().is_some(),
        "presets": presets,
        "enabled": cfg.enabled,
        "default_mic": cfg.default_mic,
        "settings": voice_settings(&cfg),
        "overrides": cfg.overrides,
        "controls": voice::CONTROLS.iter().map(|(path, group, label, min, max, step, unit)| {
            json!({ "path": path, "group": group, "label": label, "min": min, "max": max, "step": step, "unit": unit })
        }).collect::<Vec<_>>(),
        "switches": voice::SWITCHES.iter().map(|(group, path)| json!({ "group": group, "path": path })).collect::<Vec<_>>(),
        "effects": voice::effect_presets().into_iter().map(|(n, s)| json!({ "name": n, "settings": s })).collect::<Vec<_>>(),
        "effect": cfg.effect.clone().unwrap_or_else(|| "Off".into()),
        "effect_settings": effect_settings(&cfg),
        "effect_overrides": cfg.effect_overrides,
        "effect_modes": {
            "vocalMode": voice::VOCAL_MODES, "phaserMode": voice::PHASER_MODES, "shapeMode": voice::SHAPE_MODES,
            "reflectionMode": voice::REFLECTION_MODES, "inputMode": voice::INPUT_MODES,
            "feedbackMode": voice::FEEDBACK_MODES, "feedbackTimbre": voice::FEEDBACK_TIMBRES,
        },
        "preset": cfg.preset.or_else(|| presets.first().cloned()),
        "running": running,
        "error": error,
    })
}

fn set_voice(shared: &Shared, id: &str, cfg: VoiceConfig) -> Result<Value, String> {
    let (gain, state) = {
        let mut st = shared.0.lock().unwrap();
        let def = voice_def(&st, id).ok_or("this device has no Blue VO!CE")?;
        let previous = st.config.devices.get(id).and_then(|c| c.voice.clone());
        let mut cfg = cfg;
        if let Some(p) = &previous {
            cfg.effect = p.effect.clone();
            cfg.effect_overrides = p.effect_overrides.clone();
        }
        // Fine-tuning belongs to the preset it was made on.
        if previous.as_ref().map(|p| &p.preset) == Some(&cfg.preset) && cfg.overrides.is_empty() {
            cfg.overrides = previous
                .as_ref()
                .map(|p| p.overrides.clone())
                .unwrap_or_default();
        }
        st.config.devices.entry(id.to_string()).or_default().voice = Some(cfg.clone());
        st.config.save();
        if cfg.active() {
            start_voice(&mut st, id, &def, &cfg);
        } else {
            st.voice.stop(id);
        }
        if cfg.active() && cfg.default_mic {
            voice::make_default();
        } else {
            voice::release_default(&def);
        }
        // Switching on or changing preset applies the preset's input gain and the smart-lock
        // setting, as G HUB does (capture: gain 0.7 → 70 on enable).
        let was_on = previous.as_ref().is_some_and(|p| p.enabled);
        let changed = previous.and_then(|p| p.preset) != cfg.preset;
        let gain = (cfg.enabled && (changed || !was_on))
            .then(|| {
                let presets = voice::presets();
                let chosen = cfg
                    .preset
                    .as_ref()
                    .and_then(|name| presets.iter().find(|(n, _)| n == name))
                    .or(presets.first());
                chosen.and_then(|(_, s)| s["inputGain"].as_f64())
            })
            .flatten()
            .map(|g| (g * 100.0).round().clamp(0.0, 100.0) as u8);
        let notify = cfg.enabled && (changed || !was_on);
        (notify.then_some(gain), voice_state(&mut st, id))
    };
    if let Some(gain) = gain {
        ask(shared, id, |tx| Cmd::VoiceApplied(gain, tx))?;
    }
    Ok(state)
}

fn app_settings(st: &State) -> Value {
    let drivers: Vec<Value> = st
        .drivers
        .all()
        .into_iter()
        .map(|d| {
            json!({
                "name": d.name,
                "file": d.file.display().to_string(),
                "products": d.products.iter().map(|p| format!("{p:04x}")).collect::<Vec<_>>(),
            })
        })
        .collect();
    let devices: Vec<Value> = st
        .workers
        .iter()
        .map(|(id, h)| {
            let v = h.view.lock().unwrap();
            let product = u16::from_str_radix(v.product.rsplit(':').next().unwrap_or(""), 16).ok();
            let driver = product
                .and_then(|p| st.drivers.for_product(p))
                .map(|d| d.file.display().to_string());
            json!({ "id": id, "name": v.name, "product": v.product, "kind": v.kind, "driver": driver })
        })
        .collect();
    json!({
        "settings": st.config.app,
        "autostart": app::autostart(),
        "drivers": drivers,
        "devices": devices,
    })
}

/// Writes a starter definition for a connected device without one, reloads definitions and
/// reconnects the device so it picks the new one up.
fn create_driver(shared: &Shared, id: &str) -> Result<Value, String> {
    let mut st = shared.0.lock().unwrap();
    let (name, kind, product) = {
        let h = st.workers.get(id).ok_or("no such device")?;
        let v = h.view.lock().unwrap();
        let product = u16::from_str_radix(v.product.rsplit(':').next().unwrap_or(""), 16)
            .map_err(|e| e.to_string())?;
        (v.name.clone(), v.kind, product)
    };
    let file = drivers::create_starter(&name, kind, product)?;
    st.drivers = drivers::Registry::load();
    // Dropping the worker closes the device; the manager finds it again with the new file.
    st.workers.remove(id);
    Ok(json!(file.display().to_string()))
}

/// The mouse's buttons for the Assignments panel: names, what each does, the last one
/// pressed (for "press the button you want to change"), and the key names actions can use.
fn buttons(st: &State, id: &str) -> Result<Value, String> {
    let h = st.workers.get(id).ok_or("no such device")?;
    let product = {
        let v = h.view.lock().unwrap();
        u16::from_str_radix(v.product.rsplit(':').next().unwrap_or(""), 16).ok()
    };
    let labels: serde_json::Map<String, Value> = product
        .and_then(|p| st.drivers.for_product(p))
        .map(|d| {
            d.buttons
                .iter()
                .map(|b| (b.number.to_string(), json!(b.label)))
                .collect()
        })
        .unwrap_or_default();
    let last = h
        .last_button
        .lock()
        .unwrap()
        .map(|(n, at)| json!({ "button": n, "ago_ms": at.elapsed().as_millis() as u64 }));
    let assignments = st
        .config
        .devices
        .get(id)
        .map(|c| json!(c.assignments))
        .unwrap_or_else(|| json!({}));
    Ok(json!({
        "labels": labels,
        "last": last,
        "assignments": assignments,
        "keys": keys::NAMES.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        "uinput": uinput::shared().err(),
        "sounds": sounds(),
    }))
}

fn yes() -> bool {
    true
}

fn profiles(st: &State) -> Value {
    json!({
        "profiles": st.config.profiles.iter().map(|p| json!({
            "name": p.name,
            "apps": p.apps,
            "devices": p.devices.keys().collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "active": st.active,
        "auto": st.config.profile_auto,
        "focused": st.focused,
        "recent": st.recent,
    })
}

/// Imported sound clips as (category/name, path).
fn sounds() -> Vec<Value> {
    let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/share/lhub/sounds");
    let mut out = Vec::new();
    for cat in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        for f in std::fs::read_dir(cat.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = format!(
                "{} / {}",
                cat.file_name().to_string_lossy(),
                f.path().file_stem().unwrap_or_default().to_string_lossy()
            );
            out.push(json!({ "name": name, "file": f.path().display().to_string() }));
        }
    }
    out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    out
}

fn backup_dir() -> PathBuf {
    config::path().with_file_name("backups")
}

/// Saved copies of the whole configuration, newest first; names are Unix times.
fn backups() -> Value {
    let mut names: Vec<u64> = std::fs::read_dir(backup_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.path().file_stem()?.to_str()?.parse().ok())
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    json!(names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
}

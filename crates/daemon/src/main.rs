//! lhubd: owns the devices, keeps their configuration applied, and serves the UI over a
//! Unix socket with one JSON request/response per line.

mod app;
mod battery;
mod config;
mod drivers;
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

use config::{AppSettings, Config, DeviceConfig, Lighting, VoiceConfig};
use worker::{Cmd, Handle};

struct State {
    config: Config,
    drivers: drivers::Registry,
    workers: HashMap<String, Handle>,
    scanned: bool,
    voice: voice::Voice,
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
    voice::stop_stale();
    let Some(_lock) = single_instance() else {
        eprintln!("lhubd already running ({})", path.display());
        return;
    };
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
        }),
        Condvar::new(),
    ));

    let manager = shared.clone();
    std::thread::spawn(move || manage(manager));

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
        let known: Vec<String> = shared.0.lock().unwrap().workers.keys().cloned().collect();
        silent.retain(|_, at| at.elapsed() < Duration::from_secs(30));
        let skip = |id: &str| known.iter().any(|k| k == id) || silent.contains_key(id);
        let quiet = hidpp::discover::scan_each(&skip, &mut |f| {
            let mut st = shared.0.lock().unwrap();
            let cfg = st.config.devices.get(&f.id).cloned().unwrap_or_default();
            eprintln!("{}: {} connected, applying {:?}", f.id, f.name, cfg);
            let id = f.id.clone();
            let driver = st.drivers.for_product(f.device.info.product).cloned();
            let voice = cfg.voice.clone().filter(|v| v.enabled);
            st.workers
                .insert(id.clone(), worker::spawn(f, cfg, driver.clone()));
            if let (Some(v), Some(def)) = (voice, driver.and_then(|d| d.voice)) {
                start_voice(&mut st, &id, &def, &v);
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
    VoiceTestPlay {
        preset: Option<String>,
    },
    VoiceTestStop,
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
            Ok(json!(
                st.config.devices.get(&id).cloned().unwrap_or_default()
            ))
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
        Request::VoiceTestPlay { preset } => {
            let settings = match preset {
                Some(name) => Some(
                    voice::presets()
                        .into_iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, s)| s)
                        .ok_or("no such preset")?,
                ),
                None => None,
            };
            let mut st = lock.lock().unwrap();
            st.voice.play_test(settings.as_ref()).map(|_| Value::Null)
        }
        Request::VoiceTestStop => {
            lock.lock().unwrap().voice.stop_test();
            Ok(Value::Null)
        }
        Request::AppSettings => Ok(app_settings(&lock.lock().unwrap())),
        Request::SetAppSettings { settings } => {
            let mut st = lock.lock().unwrap();
            st.config.app = settings;
            st.config.save();
            Ok(app_settings(&st))
        }
        Request::SetAutostart { on } => app::set_autostart(on).map(|_| json!(app::autostart())),
        Request::ImportGhub { path } => app::import_ghub(&path).map(|t| json!(t)),
        Request::CreateDriver { id } => create_driver(shared, &id),
        Request::SetVoice {
            id,
            enabled,
            preset,
        } => set_voice(shared, &id, VoiceConfig { enabled, preset }),
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
        Request::SetDpi { id, levels, level } => {
            if levels.is_empty() || levels.len() > 5 || level >= levels.len() {
                return Err("need 1-5 DPI levels and a valid active level".into());
            }
            update(shared, &id, |c| {
                c.dpi_levels = levels;
                c.dpi_level = Some(level);
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
    let cfg = st.config.devices.entry(id.to_string()).or_default();
    change(cfg);
    let cfg = cfg.clone();
    st.config.save();
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
        let src = st.config.devices.get(from).cloned().unwrap_or_default();
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
fn start_voice(st: &mut State, id: &str, def: &drivers::VoiceDef, cfg: &VoiceConfig) {
    let presets = voice::presets();
    let chosen = cfg
        .preset
        .as_ref()
        .and_then(|name| presets.iter().find(|(n, _)| n == name))
        .or(presets.first());
    let settings = chosen.map(|(_, s)| s.clone()).unwrap_or(Value::Null);
    st.voice.start(id, def, &settings);
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
        st.config.devices.entry(id.to_string()).or_default().voice = Some(cfg.clone());
        st.config.save();
        if cfg.enabled {
            start_voice(&mut st, id, &def, &cfg);
        } else {
            st.voice.stop(id);
        }
        // A newly picked preset brings its input gain, as in G HUB (0.7 → 70 %).
        let changed = previous.and_then(|p| p.preset) != cfg.preset;
        let gain = (cfg.enabled && changed)
            .then(|| {
                voice::presets()
                    .into_iter()
                    .find(|(n, _)| Some(n) == cfg.preset.as_ref())
                    .and_then(|(_, s)| s["inputGain"].as_f64())
            })
            .flatten()
            .map(|g| (g * 100.0).round().clamp(0.0, 100.0) as u8);
        (gain, voice_state(&mut st, id))
    };
    if let Some(g) = gain {
        ask(shared, id, |tx| Cmd::SetGain(g, tx))?;
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

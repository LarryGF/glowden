//! lhubd: owns the devices, keeps their configuration applied, and serves the UI over a
//! Unix socket with one JSON request/response per line.

mod config;
mod drivers;
mod view;
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

use config::{Config, DeviceConfig, Lighting};
use worker::{Cmd, Handle};

struct State {
    config: Config,
    drivers: drivers::Registry,
    workers: HashMap<String, Handle>,
    scanned: bool,
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
            st.workers.insert(id, worker::spawn(f, cfg, driver));
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
    DeviceImage {
        id: String,
    },
    DeviceLayout {
        id: String,
    },
    LightingState {
        id: String,
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
        Request::MouseInfo { id } => ask(shared, &id, Cmd::MouseInfo),
        Request::DeviceLayout { id } => {
            let st = lock.lock().unwrap();
            let v = st.workers.get(&id).map(|h| h.view.lock().unwrap().clone());
            Ok(json!(v.and_then(|v| view::layout(
                v.model.as_deref()?,
                v.hid_usage_zones,
                v.first_view
            ))))
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

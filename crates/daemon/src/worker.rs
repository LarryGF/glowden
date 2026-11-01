//! One thread per device. It owns the device handle, applies configuration, runs
//! host-rendered animations, and exits when the device goes away.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hidpp::devices::{self, Effect as DeviceEffect, mic};
use hidpp::discover::Found;
use lighting::effects::{Pulse, Starlight, Visualizer, Wave};
use lighting::engine::{Effect, Press, Streamer};
use serde_json::{Value, json};

use crate::config::{DeviceConfig, Lighting, layer_rgb, lerp, parse_rgb, period_ms};
use crate::drivers::Driver;
use crate::view::{self, DeviceView};

pub type Reply = Sender<Result<Value, String>>;

pub enum Cmd {
    Apply(DeviceConfig),
    GetGain(Reply),
    SetGain(u8, Reply),
    SetMuted(bool, Reply),
    Release(Reply),
    MouseInfo(Reply),
}

pub struct Handle {
    pub tx: Sender<Cmd>,
    pub view: Arc<Mutex<DeviceView>>,
    pub alive: Arc<AtomicBool>,
    /// What the LEDs show, for the UI's lighting viewer.
    pub lights: Arc<Mutex<LightState>>,
}

#[derive(Default, Clone, serde::Serialize)]
pub struct LightState {
    /// "live" when we stream frames (zones are exact), "device" when the device runs the
    /// effect itself (zones hold its base color), "none" before anything was applied.
    pub mode: &'static str,
    pub effect: Option<String>,
    pub zones: std::collections::BTreeMap<u8, String>,
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

struct Animation {
    streamer: Streamer,
    effect: Box<dyn Effect>,
    presses: Option<Receiver<Press>>,
    recent: Vec<Press>,
}

pub fn spawn(mut found: Found, config: DeviceConfig, driver: Option<Driver>) -> Handle {
    let (tx, rx) = mpsc::channel();
    let view = Arc::new(Mutex::new(view::build(&mut found, driver.as_ref())));
    let alive = Arc::new(AtomicBool::new(true));
    let lights = Arc::new(Mutex::new(LightState {
        mode: "none",
        ..Default::default()
    }));
    let handle = Handle {
        tx: tx.clone(),
        view: view.clone(),
        alive: alive.clone(),
        lights: lights.clone(),
    };
    let _ = tx.send(Cmd::Apply(config));
    std::thread::Builder::new()
        .name(format!("dev {}", found.id))
        .spawn(move || {
            let id = found.id.clone();
            if let Err(e) = run(found, rx, view, lights, driver) {
                eprintln!("{id}: worker stopped: {e}");
            }
            alive.store(false, Ordering::Relaxed);
        })
        .expect("spawn device worker");
    handle
}

fn run(
    mut found: Found,
    rx: Receiver<Cmd>,
    view: Arc<Mutex<DeviceView>>,
    lights: Arc<Mutex<LightState>>,
    driver: Option<Driver>,
) -> Result<(), String> {
    let driver = driver.as_ref();
    let mut anim: Option<Animation> = None;
    let mut last_battery = Instant::now();

    loop {
        let wait = anim
            .as_ref()
            .map_or(Duration::from_secs(1), |a| a.streamer.frame_interval);
        let tick = Instant::now();
        match rx.recv_timeout(wait).map(|cmd| coalesce(cmd, &rx)) {
            Ok(cmds) => {
                for cmd in cmds {
                    let (applied, power) = match &cmd {
                        Cmd::Apply(cfg) => (
                            cfg.lighting.clone().filter(|_| cfg.power != Some(false)),
                            cfg.power,
                        ),
                        _ => (None, None),
                    };
                    handle(&mut found, driver, cmd, &mut anim)?;
                    if let Some(l) = applied {
                        record_device_lighting(&mut found, &l, anim.is_some(), &lights);
                    }
                    if let Some(false) = power {
                        let mut st = lights.lock().unwrap();
                        st.mode = "device";
                        st.effect = Some("off".into());
                        st.zones.values_mut().for_each(|c| *c = "#000000".into());
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }

        if let Some(a) = anim.as_mut()
            && tick.elapsed() >= Duration::from_millis(10)
        {
            if let Some(rx) = &a.presses {
                a.recent.extend(rx.try_iter());
            }
            a.recent.retain(|p| p.at.elapsed() < Duration::from_secs(4));
            a.streamer
                .frame(&mut found.device, a.effect.as_mut(), &a.recent)
                .map_err(|e| format!("animation: {e}"))?;
            let mut l = lights.lock().unwrap();
            l.mode = "live";
            l.zones = a
                .streamer
                .colors()
                .iter()
                .map(|(z, c)| (*z, hex(*c)))
                .collect();
        }

        if last_battery.elapsed() > Duration::from_secs(60) {
            last_battery = Instant::now();
            if let Ok((percent, charging)) = devices::battery(&mut found.device) {
                view.lock().unwrap().battery = Some(view::Battery {
                    percent,
                    charging,
                    hours: None,
                });
            }
        }
    }
}

/// Device errors from user commands go back to the caller; only a vanished device
/// (I/O error) ends the worker.
fn handle(
    found: &mut Found,
    driver: Option<&Driver>,
    cmd: Cmd,
    anim: &mut Option<Animation>,
) -> Result<(), String> {
    let dev = &mut found.device;
    let reply = |tx: Reply, r: hidpp::Result<Value>| -> Result<(), String> {
        let gone = matches!(r, Err(hidpp::Error::Io(_)));
        let _ = tx.send(r.map_err(|e| e.to_string()));
        if gone {
            Err("device disconnected".into())
        } else {
            Ok(())
        }
    };
    match cmd {
        Cmd::Apply(cfg) => {
            if let Err(e) = apply(found, driver, &cfg, anim) {
                eprintln!("{}: applying config failed: {e}", found.id);
                if matches!(e, ApplyError::Device(hidpp::Error::Io(_))) {
                    return Err("device disconnected".into());
                }
            }
            Ok(())
        }
        Cmd::GetGain(tx) => reply(tx, mic::gain(dev).map(|g| json!(g))),
        Cmd::SetGain(g, tx) => reply(tx, mic::set_gain(dev, g).map(|_| Value::Null)),
        Cmd::SetMuted(m, tx) => reply(tx, mic::set_muted(dev, m).map(|_| Value::Null)),
        Cmd::MouseInfo(tx) => {
            use devices::mouse;
            let info = (|| -> hidpp::Result<Value> {
                Ok(json!({
                    "dpi": mouse::dpi(dev)?,
                    "dpi_list": mouse::dpi_range(dev)?,
                    "report_interval": mouse::report_interval(dev)?,
                    "report_intervals": mouse::report_intervals(dev)?,
                }))
            })();
            reply(tx, info)
        }
        Cmd::Release(tx) => {
            *anim = None;
            reply(tx, devices::release_lighting(dev).map(|_| Value::Null))
        }
    }
}

#[derive(Debug)]
enum ApplyError {
    Config(String),
    Device(hidpp::Error),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ApplyError::Config(s) => write!(f, "{s}"),
            ApplyError::Device(e) => write!(f, "{e}"),
        }
    }
}

impl From<hidpp::Error> for ApplyError {
    fn from(e: hidpp::Error) -> Self {
        ApplyError::Device(e)
    }
}

impl From<String> for ApplyError {
    fn from(e: String) -> Self {
        ApplyError::Config(e)
    }
}

fn apply(
    found: &mut Found,
    driver: Option<&Driver>,
    cfg: &DeviceConfig,
    anim: &mut Option<Animation>,
) -> Result<(), ApplyError> {
    let dev = &mut found.device;
    if cfg.power == Some(false) {
        *anim = None;
        // Devices with an "off" effect use it; others (Yeti GX) go to brightness 0.
        let has_off =
            devices::cluster_effects(dev, 0).is_ok_and(|l| l.iter().any(|(_, id)| *id == 0));
        if has_off {
            devices::set_cluster_effect(dev, 0, 0x0000, &[])?;
        } else if dev.feature(hidpp::features::BRIGHTNESS).is_ok() {
            devices::set_brightness(dev, 0)?;
        }
        return Ok(());
    }
    if let Some(lighting) = &cfg.lighting {
        if lighting.is_host_rendered() {
            let effect = host_effect(lighting)?;
            // Reuse the running stream when only effect parameters changed.
            match anim {
                Some(a) => a.effect = effect,
                None => {
                    let keys = driver.and_then(|d| d.layout.keys.as_deref());
                    let positions = if keys == Some("iso-tkl") {
                        lighting::layout::iso_tkl_positions()
                    } else {
                        // No geometry known yet: lay zones out in a line.
                        devices::perkey::zones(dev)?
                            .into_iter()
                            .enumerate()
                            .map(|(i, z)| (z, i as f32, 0.0))
                            .collect()
                    };
                    let streamer = Streamer::new(dev, positions)?;
                    let presses = driver
                        .is_some_and(|d| d.input.reactive)
                        .then(|| {
                            let short = found.name.split_whitespace().next().unwrap_or(&found.name);
                            let nodes = lighting::input::find_event_nodes(short);
                            lighting::input::watch(&nodes).ok()
                        })
                        .flatten();
                    *anim = Some(Animation {
                        streamer,
                        effect,
                        presses,
                        recent: Vec::new(),
                    });
                }
            }
        } else if let Lighting::Freestyle { base, keys } = lighting {
            *anim = None;
            let zones = devices::perkey::zones(dev)?;
            devices::perkey::begin(dev)?;
            devices::perkey::fill(dev, &zones, parse_rgb(base)?)?;
            let painted = keys
                .iter()
                .filter(|(z, _)| zones.contains(z))
                .map(|(z, c)| Ok((*z, parse_rgb(c)?)))
                .collect::<Result<Vec<_>, String>>()?;
            devices::perkey::set_keys(dev, &painted)?;
            devices::perkey::commit(dev)?;
        } else if let Lighting::Firmware { id, color, glow } = &normalize(lighting) {
            *anim = None;
            let fx = driver
                .and_then(|d| d.firmware_effects.iter().find(|e| &e.id == id))
                .ok_or_else(|| format!("no firmware effect '{id}' in this device's driver file"))?;
            devices::set_cluster_effect(
                dev,
                fx.cluster,
                fx.effect_id,
                &fx.params_for(layer_rgb(color, *glow)?)?,
            )?;
        } else {
            *anim = None;
            devices::set_effect(dev, native_effect(lighting)?)?;
        }
    }
    if let Some(level) = cfg.dpi_level
        && let Some(&dpi) = cfg.dpi_levels.get(level)
    {
        devices::mouse::set_dpi(dev, dpi, level as u8 + 1)?;
    }
    if let Some(ms) = cfg.report_interval {
        devices::mouse::set_report_interval(dev, ms)?;
    }
    if let Some(b) = cfg.brightness {
        if let Some(a) = anim.as_mut() {
            a.streamer.brightness = b.min(100) as f32 / 100.0;
        }
        if dev.feature(hidpp::features::BRIGHTNESS).is_ok() {
            devices::set_brightness(dev, b)?;
        }
    }
    Ok(())
}

fn native_effect(l: &Lighting) -> Result<DeviceEffect, String> {
    Ok(match l {
        Lighting::Fixed { color } => DeviceEffect::Solid {
            rgb: parse_rgb(color)?,
        },
        Lighting::Breathing {
            color,
            speed,
            intensity,
        } => DeviceEffect::Breathe {
            rgb: parse_rgb(color)?,
            period_ms: period_ms(*speed),
            intensity: (*intensity).clamp(1, 100),
        },
        Lighting::Cycle {
            speed,
            intensity,
            saturation,
        } => DeviceEffect::Cycle {
            period_ms: period_ms(*speed),
            intensity: (*intensity).clamp(1, 100),
            saturation: *saturation,
        },
        _ => unreachable!("host-rendered effect"),
    })
}

fn host_effect(l: &Lighting) -> Result<Box<dyn Effect>, String> {
    Ok(match l {
        Lighting::Wave { speed } => Box::new(Wave {
            period: lerp(*speed, 12.0, 1.0),
            spread: 1.0,
        }),
        Lighting::Pulse {
            color,
            base,
            speed,
            width,
            fade,
            rainbow,
            glow,
            backdrop,
        } => {
            let mut p = Pulse::new(layer_rgb(color, *glow)?, layer_rgb(base, *backdrop)?);
            p.speed = lerp(*speed, 4.0, 30.0);
            p.width = lerp(*width, 0.6, 4.0);
            p.lifetime = lerp(*fade, 0.4, 3.0);
            p.hue_step = if *rainbow { 0.13 } else { 0.0 };
            Box::new(p)
        }
        Lighting::Starlight {
            star,
            sky,
            density,
            glow,
            backdrop,
        } => {
            let mut s = Starlight::new(layer_rgb(star, *glow)?, layer_rgb(sky, *backdrop)?);
            s.density = lerp(*density, 1.0, 40.0);
            Box::new(s)
        }
        Lighting::Visualizer {
            low,
            high,
            background,
            glow,
            backdrop,
        } => Box::new(
            Visualizer::new(
                layer_rgb(low, *glow)?,
                layer_rgb(high, *glow)?,
                layer_rgb(background, *backdrop)?,
            )
            .map_err(|e| format!("audio capture (pw-record): {e}"))?,
        ),
        _ => unreachable!("device-native effect"),
    })
}

/// For device-run effects, the viewer shows every zone in the effect's base color.
fn record_device_lighting(
    found: &mut Found,
    l: &Lighting,
    live: bool,
    lights: &Arc<Mutex<LightState>>,
) {
    let effect = serde_json::to_value(l)
        .ok()
        .and_then(|v| v["effect"].as_str().map(str::to_string));
    let mut st = lights.lock().unwrap();
    st.effect = effect;
    if live {
        return;
    }
    let color = match l {
        Lighting::Fixed { color }
        | Lighting::Breathing { color, .. }
        | Lighting::Ripple { color }
        | Lighting::Firmware { color, .. } => parse_rgb(color).ok(),
        Lighting::Cycle { .. } => Some([0xff, 0x2a, 0x2a]),
        _ => None,
    };
    st.mode = "device";
    st.zones.clear();
    if let Lighting::Freestyle { base, keys } = l {
        let zones = devices::perkey::zones(&mut found.device).unwrap_or_default();
        st.zones = zones
            .into_iter()
            .map(|z| (z, keys.get(&z).cloned().unwrap_or_else(|| base.clone())))
            .collect();
        st.effect = Some("freestyle".into());
        return;
    }
    if let Some(c) = color {
        let zones = devices::perkey::zones(&mut found.device).unwrap_or_else(|_| vec![1]);
        st.zones = zones.into_iter().map(|z| (z, hex(c))).collect();
    }
}

/// Old configs stored the G515 ripple as its own effect; it is now firmware effect "ripple".
fn normalize(l: &Lighting) -> Lighting {
    match l {
        Lighting::Ripple { color } => Lighting::Firmware {
            id: "ripple".into(),
            color: color.clone(),
            glow: 100,
        },
        other => other.clone(),
    }
}

/// A slider drag queues many Apply commands; only the newest one is applied, so a slow
/// wireless device doesn't replay every intermediate value. Other commands keep their order.
fn coalesce(first: Cmd, rx: &Receiver<Cmd>) -> Vec<Cmd> {
    let mut latest_apply = None;
    let mut others = Vec::new();
    for cmd in std::iter::once(first).chain(rx.try_iter()) {
        match cmd {
            Cmd::Apply(_) => latest_apply = Some(cmd),
            other => others.push(other),
        }
    }
    latest_apply.into_iter().chain(others).collect()
}

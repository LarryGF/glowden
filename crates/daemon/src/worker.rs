//! One thread per device. It owns the device handle, applies configuration, runs
//! host-rendered animations, and exits when the device goes away.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hidpp::devices::{self, Effect as DeviceEffect, mic};
use hidpp::discover::Found;
use lighting::effects::{Pulse, Screen, Sequence, Starlight, Visualizer, Wave};
use lighting::engine::{Effect, Press, Streamer};
use serde_json::{Value, json};

use crate::config::{
    Action, DeviceConfig, Lighting, MacroStep, layer_rgb, lerp, parse_rgb, period_ms,
};
use crate::drivers::Driver;
use crate::view::{self, DeviceView};

pub type Reply = Sender<Result<Value, String>>;

pub enum Cmd {
    Apply(DeviceConfig),
    GetGain(Reply),
    SetGain(u8, Reply),
    SetMuted(bool, Reply),
    GetMuted(Reply),
    /// Blue VO!CE switched on or its preset changed: what G HUB sends the mic then.
    VoiceApplied(Option<u8>, Vec<(u16, u8, Vec<u8>)>, Reply),
    Release(Reply),
    MouseInfo(Reply),
}

pub struct Handle {
    pub tx: Sender<Cmd>,
    pub view: Arc<Mutex<DeviceView>>,
    pub alive: Arc<AtomicBool>,
    /// What the LEDs show, for the UI's lighting viewer.
    pub lights: Arc<Mutex<LightState>>,
    /// DPI level last chosen with the mouse's own buttons, for the service to save.
    pub dpi_level: Arc<Mutex<Option<usize>>>,
    /// Last mouse button pressed (spy numbering) and when.
    pub last_button: Arc<Mutex<Option<(u8, Instant)>>>,
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
    /// Consecutive frames the device didn't take; a few are normal on a busy device.
    failures: u32,
}

pub fn spawn(mut found: Found, config: DeviceConfig, driver: Option<Driver>) -> Handle {
    let (tx, rx) = mpsc::channel();
    let view = Arc::new(Mutex::new(view::build(&mut found, driver.as_ref())));
    let alive = Arc::new(AtomicBool::new(true));
    let lights = Arc::new(Mutex::new(LightState {
        mode: "none",
        ..Default::default()
    }));
    let dpi_level = Arc::new(Mutex::new(None));
    let last_button = Arc::new(Mutex::new(None));
    let handle = Handle {
        tx: tx.clone(),
        view: view.clone(),
        alive: alive.clone(),
        lights: lights.clone(),
        dpi_level: dpi_level.clone(),
        last_button: last_button.clone(),
    };
    let _ = tx.send(Cmd::Apply(config));
    std::thread::Builder::new()
        .name(format!("dev {}", found.id))
        .spawn(move || {
            let id = found.id.clone();
            if let Err(e) = run(found, rx, view, lights, dpi_level, last_button, driver) {
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
    dpi_level: Arc<Mutex<Option<usize>>>,
    last_button: Arc<Mutex<Option<(u8, Instant)>>>,
    driver: Option<Driver>,
) -> Result<(), String> {
    let driver = driver.as_ref();
    let mut dpi = Buttons::new(driver, dpi_level, last_button);
    let mut anim: Option<Animation> = None;
    let mut last_battery = Instant::now();

    loop {
        let mut wait = anim
            .as_ref()
            .map_or(Duration::from_secs(1), |a| a.streamer.frame_interval);
        if dpi.spy.is_some() {
            // Button events are read between commands; keep presses responsive.
            wait = wait.min(Duration::from_millis(15));
        }
        let tick = Instant::now();
        match rx.recv_timeout(wait).map(|cmd| coalesce(cmd, &rx)) {
            Ok(cmds) => {
                for cmd in cmds {
                    if let Cmd::Apply(cfg) = &cmd {
                        dpi.configure(&mut found, cfg);
                    }
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
            if let Err(e) = a
                .streamer
                .frame(&mut found.device, a.effect.as_mut(), &a.recent)
            {
                a.failures += 1;
                if a.failures < 5 {
                    continue;
                }
                // Stop the effect instead of dropping the device: restarting it would only
                // fail the same way, in a reconnect loop.
                eprintln!("{}: live effect stopped: {e}", found.id);
                anim = None;
                lights.lock().unwrap().mode = "device";
                continue;
            }
            a.failures = 0;
            let mut l = lights.lock().unwrap();
            l.mode = "live";
            l.zones = a
                .streamer
                .colors()
                .iter()
                .map(|(z, c)| (*z, hex(*c)))
                .collect();
        }

        // Notifications the device sent: button presses, battery changes.
        let events = found.device.take_events(Duration::ZERO).unwrap_or_default();
        dpi.handle(&mut found, &events);
        if let Some((percent, charging)) = devices::battery_event(&mut found.device, &events) {
            let mut v = view.lock().unwrap();
            let hours = v.battery.as_ref().and_then(|b| b.hours);
            v.battery = Some(view::Battery {
                percent,
                charging,
                hours,
            });
        }

        // A headset switched off behind its dongle gives no I/O error, only unanswered
        // requests; it's checked more often and the worker ends so it's listed as off.
        let headset = found.device.is_centurion();
        let every = Duration::from_secs(if headset { 15 } else { 60 });
        if last_battery.elapsed() > every {
            last_battery = Instant::now();
            // A mouse that slept or reconnected forgets the spy.
            dpi.restart(&mut found);
            match devices::battery(&mut found.device) {
                Ok((percent, charging)) => {
                    view.lock().unwrap().battery = Some(view::Battery {
                        percent,
                        charging,
                        hours: None,
                    });
                }
                Err(e) if headset => return Err(format!("switched off ({e})")),
                Err(_) => {}
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
        Cmd::GetMuted(tx) => reply(tx, mic::muted(dev).map(|m| json!(m))),
        Cmd::VoiceApplied(gain, requests, tx) => {
            let r = (|| {
                if let Some(g) = gain {
                    mic::set_gain(dev, g)?;
                }
                // The definition's extra requests (Yeti GX: G HUB's smart-lock setting).
                for (feature, function, params) in &requests {
                    dev.call(*feature, *function, params)?;
                }
                Ok(())
            })();
            reply(tx, r.map(|_| Value::Null))
        }
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
                        failures: 0,
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
        Lighting::Sequence {
            steps,
            blend,
            spread,
        } => {
            if steps.is_empty() || steps.len() > 64 {
                return Err("a sequence needs 1 to 64 steps".into());
            }
            let steps = steps
                .iter()
                .map(|s| Ok((parse_rgb(&s.color)?, s.ms.clamp(50, 60_000) as f32 / 1000.0)))
                .collect::<Result<Vec<_>, String>>()?;
            Box::new(Sequence {
                steps,
                blend: *blend,
                spread: *spread as f32 / 100.0 * 3.0,
            })
        }
        Lighting::Screen { speed, boost } => {
            let mut s =
                Screen::new().map_err(|e| format!("screen capture (glowden-sampler): {e}"))?;
            s.follow = lerp(*speed, 0.05, 0.9);
            s.saturation = *boost as f32 / 50.0;
            Box::new(s)
        }
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

/// Mouse buttons handled on the host, through the button spy (0x8110): assigned buttons are
/// silenced on the mouse (its button routing, as G HUB does) and performed here, and the DPI
/// buttons host mode leaves to the host step through the DPI levels.
struct Buttons {
    up: u16,
    down: u16,
    shift: u16,
    /// Feature index of the button spy once started.
    spy: Option<u8>,
    count: u8,
    levels: Vec<u16>,
    level: usize,
    held: u16,
    chosen: Arc<Mutex<Option<usize>>>,
    assignments: BTreeMap<u8, Action>,
    /// Unassigned routing (driver file, else factory) and what the mouse has now.
    base: Option<Vec<u8>>,
    /// Left-handed: buttons 1 and 2 send each other's click.
    swap: bool,
    routed: Option<Vec<u8>>,
    /// Keys each held button keeps down, released with it.
    holding: HashMap<u8, Vec<u16>>,
    shift_dpi: Option<u16>,
    spy_failed: bool,
    /// Last button pressed, for the app's "press the button" detection.
    last: Arc<Mutex<Option<(u8, Instant)>>>,
}

impl Buttons {
    fn new(
        driver: Option<&Driver>,
        chosen: Arc<Mutex<Option<usize>>>,
        last: Arc<Mutex<Option<(u8, Instant)>>>,
    ) -> Buttons {
        let input = driver.map(|d| d.input.clone()).unwrap_or_default();
        Buttons {
            up: input.dpi_up.unwrap_or(0),
            down: input.dpi_down.unwrap_or(0),
            shift: input.dpi_shift.unwrap_or(0),
            spy: None,
            count: 0,
            levels: Vec::new(),
            level: 0,
            held: 0,
            chosen,
            assignments: BTreeMap::new(),
            base: input.routing.clone(),
            swap: false,
            routed: None,
            holding: HashMap::new(),
            shift_dpi: None,
            spy_failed: false,
            last,
        }
    }

    fn configure(&mut self, found: &mut Found, cfg: &DeviceConfig) {
        self.levels = cfg.dpi_levels.clone();
        self.shift_dpi = cfg.dpi_shift;
        self.level = cfg
            .dpi_level
            .unwrap_or(0)
            .min(self.levels.len().saturating_sub(1));
        self.assignments = cfg.assignments.clone();
        self.swap = cfg.swap_buttons;
        if self.spy.is_none() {
            self.restart(found);
        }
        self.route(found);
    }

    /// Starts the spy on mice that have it; also after the mouse slept or reconnected.
    fn restart(&mut self, found: &mut Found) {
        if found.device.feature(devices::buttons::SPY).is_err() {
            return;
        }
        match devices::buttons::start_spy(&mut found.device) {
            Ok(index) => {
                self.spy = Some(index);
                self.spy_failed = false;
                self.count = devices::buttons::count(&mut found.device).unwrap_or(0);
                self.routed = devices::buttons::remapping(&mut found.device).ok();
                self.route(found);
            }
            Err(e) => {
                // Asleep mice fail every retry; say it once.
                if self.spy.is_some() || !self.spy_failed {
                    eprintln!("{}: button spy: {e}", found.id);
                }
                self.spy_failed = true;
                self.spy = None;
            }
        }
    }

    /// Assigned buttons send nothing to the PC; the rest keep the base routing. Written only
    /// when the mouse has something else.
    fn route(&mut self, found: &mut Found) {
        if self.spy.is_none() || self.count == 0 {
            return;
        }
        let map: Vec<u8> = (1..=self.count)
            .map(|n| {
                let base = self
                    .base
                    .as_ref()
                    .and_then(|b| b.get(n as usize - 1).copied());
                if self.assignments.contains_key(&n) {
                    0
                } else {
                    base.unwrap_or(n)
                }
            })
            .collect();
        if self.routed.as_ref() == Some(&map) {
            return;
        }
        match devices::buttons::set_remapping(&mut found.device, &map) {
            Ok(()) => self.routed = Some(map),
            Err(e) => eprintln!("{}: button routing: {e}", found.id),
        }
    }

    fn handle(&mut self, found: &mut Found, events: &[[u8; hidpp::REPLY_LEN]]) {
        let Some(index) = self.spy else { return };
        for msg in events {
            let Some(mask) = devices::buttons::pressed(index, msg) else {
                continue;
            };
            let pressed = mask & !self.held;
            let released = self.held & !mask;
            self.held = mask;
            for n in 1..=16u8 {
                let bit = 1u16 << (n - 1);
                if pressed & bit != 0 {
                    *self.last.lock().unwrap() = Some((n, Instant::now()));
                    self.button(found, n, bit, true);
                } else if released & bit != 0 {
                    self.button(found, n, bit, false);
                }
            }
        }
    }

    fn button(&mut self, found: &mut Found, n: u8, bit: u16, down: bool) {
        let action = self.assignments.get(&n).cloned().or_else(|| {
            // Unassigned DPI buttons keep their usual job.
            let dpi = |a: &str| Some(Action::Dpi { action: a.into() });
            if bit & self.up != 0 {
                dpi("up")
            } else if bit & self.down != 0 {
                dpi("down")
            } else if bit & self.shift != 0 {
                dpi("shift")
            } else {
                None
            }
        });
        let Some(action) = action else { return };
        if let Action::Dpi { action } = &action {
            self.dpi(found, action, down);
            return;
        }
        if let Err(e) = self.perform(n, &action, down) {
            eprintln!("{}: button {n}: {e}", found.id);
        }
    }

    fn perform(&mut self, n: u8, action: &Action, down: bool) -> Result<(), String> {
        let codes = |names: &[String]| -> Result<Vec<u16>, String> {
            names
                .iter()
                .map(|k| crate::keys::code(k).ok_or_else(|| format!("unknown key {k}")))
                .collect()
        };
        match action {
            Action::Keys { keys } => {
                let input = crate::uinput::shared()?;
                let mut input = input.lock().unwrap();
                if down {
                    let codes = codes(keys)?;
                    for &c in &codes {
                        input.key(c, true).map_err(|e| e.to_string())?;
                    }
                    self.holding.insert(n, codes);
                } else if let Some(codes) = self.holding.remove(&n) {
                    for &c in codes.iter().rev() {
                        input.key(c, false).map_err(|e| e.to_string())?;
                    }
                }
            }
            Action::MouseButton { button } => {
                let code = codes(std::slice::from_ref(button))?[0];
                let input = crate::uinput::shared()?;
                input
                    .lock()
                    .unwrap()
                    .key(code, down)
                    .map_err(|e| e.to_string())?;
            }
            Action::Scroll { direction } if down => {
                let (axis, notches) = match direction.as_str() {
                    "up" => (crate::uinput::REL_WHEEL, 1),
                    "down" => (crate::uinput::REL_WHEEL, -1),
                    "left" => (crate::uinput::REL_HWHEEL, -1),
                    "right" => (crate::uinput::REL_HWHEEL, 1),
                    other => return Err(format!("unknown scroll direction {other}")),
                };
                let input = crate::uinput::shared()?;
                input
                    .lock()
                    .unwrap()
                    .scroll(axis, notches)
                    .map_err(|e| e.to_string())?;
            }
            Action::Command { command } if down => {
                std::process::Command::new("sh")
                    .args(["-c", command])
                    .stdin(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            Action::Sound { file } if down => {
                std::process::Command::new("pw-play")
                    .arg(file)
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| format!("cannot play {file}: {e}"))?;
            }
            Action::Macro { steps } if down => {
                // Played on its own thread so pauses don't hold up the device.
                let input = crate::uinput::shared()?;
                let steps: Vec<Result<Vec<u16>, u32>> = steps
                    .iter()
                    .map(|s| match s {
                        MacroStep::Keys { keys } => codes(keys).map(Ok),
                        MacroStep::Pause { ms } => Ok(Err(*ms)),
                    })
                    .collect::<Result<_, _>>()?;
                std::thread::spawn(move || {
                    for step in steps {
                        match step {
                            Ok(codes) => {
                                let mut input = input.lock().unwrap();
                                for &c in &codes {
                                    let _ = input.key(c, true);
                                }
                                for &c in codes.iter().rev() {
                                    let _ = input.key(c, false);
                                }
                            }
                            Err(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
                        }
                    }
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn dpi(&mut self, found: &mut Found, action: &str, down: bool) {
        if self.levels.is_empty() {
            return;
        }
        let last = self.levels.len() - 1;
        if action == "shift"
            && down
            && let Some(dpi) = self.shift_dpi
        {
            if found.device.host_mode()
                && let Err(e) = devices::mouse::set_dpi(&mut found.device, dpi, 1)
            {
                eprintln!("{}: dpi shift: {e}", found.id);
            }
            return;
        }
        let target = match (action, down) {
            ("shift", true) => Some(0),
            ("shift", false) => Some(self.level),
            ("up", true) => {
                self.level = (self.level + 1).min(last);
                Some(self.level)
            }
            ("down", true) => {
                self.level = self.level.saturating_sub(1);
                Some(self.level)
            }
            ("cycle", true) => {
                self.level = if self.level >= last {
                    0
                } else {
                    self.level + 1
                };
                Some(self.level)
            }
            _ => None,
        };
        if action != "shift" && target.is_some() {
            *self.chosen.lock().unwrap() = Some(self.level);
        }
        // On its onboard profile the mouse steps DPI itself.
        if !found.device.host_mode() {
            return;
        }
        if let Some(l) = target
            && let Err(e) = devices::mouse::set_dpi(&mut found.device, self.levels[l], l as u8 + 1)
        {
            eprintln!("{}: dpi button: {e}", found.id);
        }
    }
}

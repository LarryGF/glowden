//! Our own lighting effects. Colors are computed per key from its position and time.

use std::time::Instant;

use crate::engine::{Effect, Key, Press, Rgb};

/// Hue (0..1), saturation, value → RGB.
pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as u32;
    let f = h - h.floor();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

fn scale(c: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    [
        (c[0] as f32 * k) as u8,
        (c[1] as f32 * k) as u8,
        (c[2] as f32 * k) as u8,
    ]
}

/// Blend from `a` toward `b` by `k` (0..1).
pub fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    [ch(a[0], b[0]), ch(a[1], b[1]), ch(a[2], b[2])]
}

fn add(a: Rgb, b: Rgb) -> Rgb {
    [
        a[0].saturating_add(b[0]),
        a[1].saturating_add(b[1]),
        a[2].saturating_add(b[2]),
    ]
}

/// A rainbow band sweeping across the board.
pub struct Wave {
    /// Seconds for one full sweep of hues.
    pub period: f32,
    /// How many hue cycles fit across the board width.
    pub spread: f32,
}

impl Effect for Wave {
    fn color(&mut self, key: &Key, t: f32, _: &[Press], _: Instant) -> Rgb {
        // Hue is quantized to 1/48 steps so neighbouring keys share colors and go out as
        // ranges; a smooth gradient would update every key every frame.
        let h = key.x / crate::layout::SIZE.0 * self.spread - t / self.period;
        hsv((h * 48.0).round() / 48.0, 1.0, 1.0)
    }
}

/// Each key press launches a ring that travels outward from that key and fades,
/// over a dim base color.
pub struct Pulse {
    pub color: Rgb,
    pub base: Rgb,
    /// Ring speed in key units per second.
    pub speed: f32,
    /// Ring thickness in key units.
    pub width: f32,
    /// Seconds until a ring has fully faded.
    pub lifetime: f32,
    /// Shifts each successive ring's hue by this much (0 keeps `color`).
    pub hue_step: f32,
    positions: Vec<(u8, f32, f32)>,
}

impl Pulse {
    pub fn new(color: Rgb, base: Rgb) -> Self {
        Pulse {
            color,
            base,
            speed: 14.0,
            width: 1.6,
            lifetime: 1.4,
            hue_step: 0.0,
            positions: crate::layout::iso_tkl_positions(),
        }
    }

    fn origin(&self, zone: u8) -> Option<(f32, f32)> {
        self.positions
            .iter()
            .find(|(z, ..)| *z == zone)
            .map(|&(_, x, y)| (x, y))
    }
}

impl Effect for Pulse {
    fn color(&mut self, key: &Key, _t: f32, presses: &[Press], now: Instant) -> Rgb {
        let mut out = self.base;
        for (n, press) in presses.iter().enumerate() {
            let age = now.duration_since(press.at).as_secs_f32();
            if age > self.lifetime {
                continue;
            }
            let Some((ox, oy)) = self.origin(press.zone) else {
                continue;
            };
            let dist = ((key.x - ox).powi(2) + ((key.y - oy) * 1.1).powi(2)).sqrt();
            let radius = age * self.speed;
            // Bright at the ring front, with a soft trailing edge; the pressed key flashes.
            let edge = 1.0 - ((dist - radius).abs() / self.width).min(1.0);
            let flash = if press.zone == key.zone {
                1.0 - age / 0.35
            } else {
                0.0
            };
            let fade = 1.0 - age / self.lifetime;
            let k = (edge * fade).max(flash);
            if k > 0.0 {
                let c = if self.hue_step != 0.0 {
                    hsv(n as f32 * self.hue_step, 1.0, 1.0)
                } else {
                    self.color
                };
                out = mix(out, c, k);
            }
        }
        out
    }
}

/// Random keys fade in and out like stars over a background.
pub struct Starlight {
    pub star: Rgb,
    pub sky: Rgb,
    /// New stars per second across the board.
    pub density: f32,
    pub twinkle: f32,
    stars: Vec<(u8, f32)>,
    rng: u64,
    last_t: f32,
}

impl Starlight {
    pub fn new(star: Rgb, sky: Rgb) -> Self {
        Starlight {
            star,
            sky,
            density: 12.0,
            twinkle: 1.2,
            stars: Vec::new(),
            rng: 0x9e3779b97f4a7c15,
            last_t: 0.0,
        }
    }

    fn next(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
}

impl Effect for Starlight {
    fn begin_frame(&mut self, t: f32) {
        let dt = (t - self.last_t).max(0.0);
        self.last_t = t;
        let expected = self.density * dt;
        let mut n = expected.floor() as u32;
        if (self.next() % 1000) as f32 / 1000.0 < expected.fract() {
            n += 1;
        }
        for _ in 0..n {
            let zone = (self.next() % 0x70) as u8 + 1;
            self.stars.push((zone, t));
        }
        let life = self.twinkle;
        self.stars.retain(|&(_, born)| t - born < life);
    }

    fn color(&mut self, key: &Key, t: f32, _: &[Press], _: Instant) -> Rgb {
        let mut k: f32 = 0.0;
        for &(zone, born) in &self.stars {
            if zone == key.zone {
                let a = (t - born) / self.twinkle;
                k = k.max((a * std::f32::consts::PI).sin());
            }
        }
        add(scale(self.sky, 1.0 - k), scale(self.star, k))
    }
}

/// Spectrum bars: the board's width is split into frequency bands (bass on the left) and
/// each band lights its column from the bottom row up, blending `low` → `high` with height.
pub struct Visualizer {
    pub low: Rgb,
    pub high: Rgb,
    pub background: Rgb,
    meter: crate::audio::AudioMeter,
    levels: Vec<f32>,
}

impl Visualizer {
    pub fn new(low: Rgb, high: Rgb, background: Rgb) -> std::io::Result<Self> {
        let meter = crate::audio::AudioMeter::start(16)?;
        Ok(Visualizer {
            low,
            high,
            background,
            meter,
            levels: vec![0.0; 16],
        })
    }
}

impl Effect for Visualizer {
    fn begin_frame(&mut self, _t: f32) {
        self.levels.clone_from(&self.meter.bands.lock().unwrap());
    }

    fn color(&mut self, key: &Key, _t: f32, _: &[Press], _: Instant) -> Rgb {
        let (w, h) = crate::layout::SIZE;
        let band = ((key.x / w) * self.levels.len() as f32) as usize;
        let level = self.levels[band.min(self.levels.len() - 1)];
        // Height of this key from the bottom edge, 0 (bottom row) .. 1 (top row).
        let height = 1.0 - key.y / h;
        if level > height {
            mix(self.low, self.high, height)
        } else {
            self.background
        }
    }
}

/// Screen sampler: each key shows the color of the part of the screen at its place on the
/// board, smoothed so the lights follow the picture without flicker.
pub struct Screen {
    /// 0..1: how quickly a key follows a change.
    pub follow: f32,
    /// Extra saturation, 0 = as on screen.
    pub saturation: f32,
    sampler: std::sync::Arc<crate::screen::ScreenSampler>,
    grid: Vec<Rgb>,
    shown: std::collections::HashMap<u8, [f32; 3]>,
}

impl Screen {
    pub fn new() -> std::io::Result<Self> {
        Ok(Screen {
            follow: 0.35,
            saturation: 0.0,
            sampler: crate::screen::ScreenSampler::shared()?,
            grid: Vec::new(),
            shown: Default::default(),
        })
    }
}

impl Effect for Screen {
    fn begin_frame(&mut self, _t: f32) {
        self.grid.clone_from(&self.sampler.grid.lock().unwrap());
    }

    fn color(&mut self, key: &Key, _t: f32, _: &[Press], _: Instant) -> Rgb {
        use crate::screen::{COLS, ROWS};
        let (w, h) = crate::layout::SIZE;
        let c = ((key.x / w) * COLS as f32).clamp(0.0, COLS as f32 - 1.0) as usize;
        let r = ((key.y / h) * ROWS as f32).clamp(0.0, ROWS as f32 - 1.0) as usize;
        let mut target = self.grid.get(r * COLS + c).copied().unwrap_or([0; 3]);
        if self.saturation > 0.0 {
            let grey = (target[0] as f32 + target[1] as f32 + target[2] as f32) / 3.0;
            for ch in &mut target {
                *ch =
                    (grey + (*ch as f32 - grey) * (1.0 + self.saturation)).clamp(0.0, 255.0) as u8;
            }
        }
        let s = self.shown.entry(key.zone).or_insert([0.0; 3]);
        for i in 0..3 {
            s[i] += (target[i] as f32 - s[i]) * self.follow;
        }
        [s[0] as u8, s[1] as u8, s[2] as u8]
    }
}

/// A user-made color sequence: each step holds its color for its time, optionally blending
/// into the next. With `spread`, the sequence runs across the board from left to right.
pub struct Sequence {
    /// (color, seconds) per step.
    pub steps: Vec<(Rgb, f32)>,
    pub blend: bool,
    /// Seconds of delay from the left edge to the right edge.
    pub spread: f32,
}

impl Effect for Sequence {
    fn color(&mut self, key: &Key, t: f32, _: &[Press], _: Instant) -> Rgb {
        let total: f32 = self.steps.iter().map(|s| s.1).sum();
        if self.steps.is_empty() || total <= 0.0 {
            return [0; 3];
        }
        let delay = key.x / crate::layout::SIZE.0 * self.spread;
        let mut at = (t - delay).rem_euclid(total);
        for (i, &(color, secs)) in self.steps.iter().enumerate() {
            if at < secs {
                if !self.blend {
                    return color;
                }
                let next = self.steps[(i + 1) % self.steps.len()].0;
                return mix(color, next, at / secs);
            }
            at -= secs;
        }
        self.steps[0].0
    }
}

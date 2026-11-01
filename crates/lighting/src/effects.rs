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
                out = add(out, scale(c, k));
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

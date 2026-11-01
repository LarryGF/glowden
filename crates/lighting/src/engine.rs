//! Renders effects into per-zone frames and streams the changes to a device.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use hidpp::Device;
use hidpp::devices::perkey;

pub type Rgb = [u8; 3];

/// A key press the effect can react to: zone and when it happened.
#[derive(Clone, Copy, Debug)]
pub struct Press {
    pub zone: u8,
    pub at: Instant,
}

pub struct Key {
    pub zone: u8,
    pub x: f32,
    pub y: f32,
}

pub trait Effect: Send {
    /// Color of `key` at `t` seconds since the effect started.
    fn color(&mut self, key: &Key, t: f32, presses: &[Press], now: Instant) -> Rgb;
    /// Called once per frame before any `color` call.
    fn begin_frame(&mut self, _t: f32) {}
}

pub struct Streamer {
    keys: Vec<Key>,
    shown: HashMap<u8, Rgb>,
    started: Instant,
    pub frame_interval: Duration,
    /// 0..1 scale applied to every rendered color; device brightness doesn't affect
    /// host-painted frames.
    pub brightness: f32,
}

impl Streamer {
    /// `positions` comes from the layout; zones the device lacks are dropped.
    pub fn new(dev: &mut Device, positions: Vec<(u8, f32, f32)>) -> hidpp::Result<Self> {
        let zones = perkey::zones(dev)?;
        let keys = positions
            .into_iter()
            .filter(|(z, ..)| zones.contains(z))
            .map(|(zone, x, y)| Key { zone, x, y })
            .collect();
        perkey::begin(dev)?;
        perkey::fill(dev, &zones, [0, 0, 0])?;
        perkey::commit(dev)?;
        Ok(Streamer {
            keys,
            shown: zones.into_iter().map(|z| (z, [0, 0, 0])).collect(),
            started: Instant::now(),
            frame_interval: Duration::from_millis(40),
            brightness: 1.0,
        })
    }

    /// Renders one frame and sends only the zones whose color changed. Returns the
    /// number of zones updated.
    pub fn frame(
        &mut self,
        dev: &mut Device,
        effect: &mut dyn Effect,
        presses: &[Press],
    ) -> hidpp::Result<usize> {
        let now = Instant::now();
        let t = now.duration_since(self.started).as_secs_f32();
        effect.begin_frame(t);
        let mut changed: Vec<(u8, Rgb)> = Vec::new();
        for key in &self.keys {
            let c = effect.color(key, t, presses, now);
            let c = if self.brightness < 1.0 {
                c.map(|v| (v as f32 * self.brightness).round() as u8)
            } else {
                c
            };
            if self.shown.get(&key.zone) != Some(&c) {
                changed.push((key.zone, c));
            }
        }
        if changed.is_empty() {
            return Ok(0);
        }
        changed.sort_by_key(|(z, _)| *z);

        // Consecutive zones with the same color go out as one range.
        let mut ranges: Vec<(u8, u8, Rgb)> = Vec::new();
        let mut singles: Vec<(u8, Rgb)> = Vec::new();
        let mut i = 0;
        while i < changed.len() {
            let (start, color) = changed[i];
            let mut j = i;
            while j + 1 < changed.len()
                && changed[j + 1].0 == changed[j].0 + 1
                && changed[j + 1].1 == color
            {
                j += 1;
            }
            if j > i {
                ranges.push((start, changed[j].0, color));
            } else {
                singles.push((start, color));
            }
            i = j + 1;
        }
        perkey::set_ranges(dev, &ranges)?;
        perkey::set_keys(dev, &singles)?;
        perkey::commit(dev)?;
        for (z, c) in &changed {
            self.shown.insert(*z, *c);
        }
        Ok(changed.len())
    }
}

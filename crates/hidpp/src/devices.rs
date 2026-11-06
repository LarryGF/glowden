//! Device-level operations decoded from G HUB USB captures.

use crate::features::{BRIGHTNESS, MICROPHONE, ONBOARD_PROFILES, PROFILE_MANAGEMENT, RGB_EFFECTS};
use crate::{Device, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    Solid {
        rgb: [u8; 3],
    },
    Breathe {
        rgb: [u8; 3],
        period_ms: u16,
        intensity: u8,
    },
    Cycle {
        period_ms: u16,
        intensity: u8,
        saturation: u8,
    },
}

fn has_feature(dev: &mut Device, id: u16) -> bool {
    dev.feature(id).is_ok()
}

/// Devices with onboard profiles refuse lighting writes (error 0x05) until the host takes
/// over: profiles go to host mode, then RGB effects function 5 switches LEDs to software
/// control (mode 3, flag 0x04 = firmware keeps watching for idle).
pub fn claim_lighting(dev: &mut Device) -> Result<()> {
    if dev.lighting_claimed {
        return Ok(());
    }
    if has_feature(dev, PROFILE_MANAGEMENT) {
        dev.call(PROFILE_MANAGEMENT, 6, &[0x05])?;
    } else if has_feature(dev, ONBOARD_PROFILES) {
        dev.call(ONBOARD_PROFILES, 1, &[0x02])?;
    } else {
        dev.lighting_claimed = true;
        return Ok(());
    }
    dev.call(RGB_EFFECTS, 5, &[0x01, 0x03, 0x04])?;
    dev.lighting_claimed = true;
    Ok(())
}

/// Hands lighting (and onboard profiles) back to the device firmware.
pub fn release_lighting(dev: &mut Device) -> Result<()> {
    dev.lighting_claimed = false;
    dev.call(RGB_EFFECTS, 5, &[0x01, 0x00, 0x00])?;
    if has_feature(dev, PROFILE_MANAGEMENT) {
        dev.call(PROFILE_MANAGEMENT, 6, &[0x03])?;
    } else if has_feature(dev, ONBOARD_PROFILES) {
        dev.call(ONBOARD_PROFILES, 1, &[0x01])?;
    }
    Ok(())
}

/// Index of the first effect on cluster 0 whose id is in `ids`.
fn effect_index(dev: &mut Device, ids: &[u16]) -> Result<u8> {
    let effects = cluster_effects(dev, 0)?;
    ids.iter()
        .find_map(|id| effects.iter().find(|(_, e)| e == id).map(|(i, _)| *i))
        .ok_or(crate::Error::UnsupportedFeature(RGB_EFFECTS))
}

/// RGB effects (0x8071) function 1 on cluster 0. The trailing 0x01 makes the device keep it.
pub fn set_effect(dev: &mut Device, effect: Effect) -> Result<()> {
    claim_lighting(dev)?;
    let mut p = [0u8; 13];
    match effect {
        Effect::Solid { rgb } => {
            p[1] = effect_index(dev, &[effect_id::FIXED])?;
            p[2..5].copy_from_slice(&rgb);
            p[5] = 0x02;
        }
        Effect::Breathe {
            rgb,
            period_ms,
            intensity,
        } => {
            p[1] = effect_index(dev, &[effect_id::BREATHING])?;
            p[2..5].copy_from_slice(&rgb);
            p[5..7].copy_from_slice(&period_ms.to_be_bytes());
            p[8] = intensity;
        }
        Effect::Cycle {
            period_ms,
            intensity,
            saturation,
        } => {
            p[1] = effect_index(dev, &[effect_id::CYCLE_ALT, effect_id::CYCLE])?;
            p[3] = saturation;
            p[8..10].copy_from_slice(&period_ms.to_be_bytes());
            p[10] = intensity;
        }
    }
    p[12] = 0x01;
    dev.call(RGB_EFFECTS, 1, &p)?;
    Ok(())
}

/// Brightness (0x8040): function 0 reports the maximum level (0xff on the Yeti GX, 100 on
/// the G515), function 2 sets the level, and on v1 G HUB follows with function 4 = 1.
pub fn set_brightness(dev: &mut Device, percent: u8) -> Result<()> {
    let info = dev.call(BRIGHTNESS, 0, &[])?;
    let max = u16::from_be_bytes([info[4], info[5]]).max(1);
    let level = (percent.min(100) as u32 * max as u32 / 100) as u16;
    dev.call(BRIGHTNESS, 2, &level.to_be_bytes())?;
    // Function 4 only exists from v1 (Yeti GX); the G515's v0 answers "invalid function".
    match dev.call(BRIGHTNESS, 4, &[0x01]) {
        Err(crate::Error::Device(_, 0x07)) => Ok(()),
        r => r.map(|_| ()),
    }
}

pub mod mic {
    use super::*;

    pub fn gain(dev: &mut Device) -> Result<u8> {
        Ok(dev.call(MICROPHONE, 2, &[0x00, 0x01, 0x00])?[7])
    }

    pub fn set_gain(dev: &mut Device, gain: u8) -> Result<()> {
        dev.call(MICROPHONE, 3, &[0x00, 0x01, gain.min(100)])?;
        Ok(())
    }

    /// Same register as `set_muted`: 1 = live, 0 = muted (read back from the Yeti GX).
    pub fn muted(dev: &mut Device) -> Result<bool> {
        Ok(dev.call(MICROPHONE, 2, &[0x00, 0x00, 0x00])?[7] == 0)
    }

    pub fn set_muted(dev: &mut Device, muted: bool) -> Result<()> {
        dev.call(MICROPHONE, 3, &[0x00, 0x00, if muted { 0 } else { 1 }])?;
        Ok(())
    }
}

/// Unified battery (0x1004) function 1: [state of charge %, level flags, charging status, ...].
/// (percent, charging) from unified battery `0x1004` fn1: [percent, level, status, external
/// power]; status 1–3 = charging (slow/fast/wireless).
pub fn battery(dev: &mut Device) -> Result<(u8, bool)> {
    if dev.is_centurion() {
        // Headsets: centurion 0x0104 fn0 getBatteryStatus = [percent, ?, status]; the
        // percent was seen draining on the G522, status 0 while unplugged.
        let r = dev.call(CENTURION_BATTERY, 0, &[])?;
        return Ok((r[4], r[6] != 0));
    }
    let r = dev.call(crate::features::UNIFIED_BATTERY, 1, &[])?;
    Ok((r[4], matches!(r[6], 1..=3)))
}

pub const CENTURION_BATTERY: u16 = 0x0104;

/// The newest battery change among device notifications (`0x1004` event 0, same layout as fn1).
pub fn battery_event(dev: &mut Device, events: &[[u8; crate::REPLY_LEN]]) -> Option<(u8, bool)> {
    if events.is_empty() {
        return None;
    }
    let index = dev.feature(crate::features::UNIFIED_BATTERY).ok()?;
    events
        .iter()
        .rev()
        .find(|m| m[2] == index && m[3] >> 4 == 0)
        .map(|m| (m[4], matches!(m[6], 1..=3)))
}

/// Effect ids from RGB effects function 0; the index of an effect differs per device.
pub mod effect_id {
    pub const OFF: u16 = 0x0000;
    pub const FIXED: u16 = 0x0001;
    pub const CYCLE: u16 = 0x0003;
    pub const BREATHING: u16 = 0x000a;
    pub const CYCLE_ALT: u16 = 0x0015;
}

/// (effect index, effect id) for every effect of `cluster`, from RGB effects function 0.
/// Reply to [cluster, 0xff]: [cluster, _, location (2), effect count, ...];
/// reply to [cluster, index]: [cluster, index, id (2), capabilities (2), period (2)].
pub fn cluster_effects(dev: &mut Device, cluster: u8) -> Result<Vec<(u8, u16)>> {
    if let Some(list) = dev.effect_cache.get(&cluster) {
        return Ok(list.clone());
    }
    let list = read_cluster_effects(dev, cluster)?;
    dev.effect_cache.insert(cluster, list.clone());
    Ok(list)
}

fn read_cluster_effects(dev: &mut Device, cluster: u8) -> Result<Vec<(u8, u16)>> {
    let id_at = |dev: &mut Device, i: u8| -> Result<u16> {
        let r = dev.call(RGB_EFFECTS, 0, &[cluster, i, 0x00])?;
        Ok(u16::from_be_bytes([r[6], r[7]]))
    };
    if cluster == 0xff {
        // The all-clusters address has no count query; walk indices until the device refuses.
        let mut out = Vec::new();
        for i in 0..32 {
            match id_at(dev, i) {
                Ok(id) => out.push((i, id)),
                Err(crate::Error::Device(..)) => break,
                Err(e) => return Err(e),
            }
        }
        return Ok(out);
    }
    let count = dev.call(RGB_EFFECTS, 0, &[cluster, 0xff, 0x00])?[8];
    (0..count).map(|i| Ok((i, id_at(dev, i)?))).collect()
}

/// Per-key lighting (0x8081), as G HUB streams it: colors are staged with function 1
/// (up to 4 × [zone, r, g, b]) or function 5 (up to 3 × [first, last, r, g, b]) and shown
/// together by function 7.
pub mod perkey {
    use crate::features::{PER_KEY_LIGHTING_V2, RGB_EFFECTS};
    use crate::{Device, Result};

    /// Zone ids the device has, from the three 16-byte bitmaps returned by function 0.
    pub fn zones(dev: &mut Device) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for page in 0..3u8 {
            let r = dev.call(PER_KEY_LIGHTING_V2, 0, &[0x00, page])?;
            for (byte, bits) in r[6..20].iter().enumerate() {
                for bit in 0..8 {
                    if bits & (1 << bit) != 0 {
                        let zone = page as usize * 112 + byte * 8 + bit;
                        if zone > 0 && zone < 256 {
                            out.push(zone as u8);
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// Switches the device's effect engine over to host-painted frames, as G HUB does
    /// before streaming (all-zones effect slot 1 with empty parameters).
    pub fn begin(dev: &mut Device) -> Result<()> {
        crate::devices::claim_lighting(dev)?;
        let mut p = [0u8; 13];
        p[0] = 0xff;
        p[1] = 0x01;
        p[12] = 0x01;
        dev.call(RGB_EFFECTS, 1, &p)?;
        Ok(())
    }

    pub fn set_keys(dev: &mut Device, keys: &[(u8, [u8; 3])]) -> Result<()> {
        for chunk in keys.chunks(4) {
            let mut p = Vec::with_capacity(16);
            for (zone, rgb) in chunk {
                p.push(*zone);
                p.extend_from_slice(rgb);
            }
            dev.call(PER_KEY_LIGHTING_V2, 1, &p)?;
        }
        Ok(())
    }

    pub fn set_ranges(dev: &mut Device, ranges: &[(u8, u8, [u8; 3])]) -> Result<()> {
        for chunk in ranges.chunks(3) {
            let mut p = Vec::with_capacity(15);
            for (first, last, rgb) in chunk {
                p.extend_from_slice(&[*first, *last]);
                p.extend_from_slice(rgb);
            }
            dev.call(PER_KEY_LIGHTING_V2, 5, &p)?;
        }
        Ok(())
    }

    /// Contiguous runs of zone ids, for painting with function 5.
    pub fn runs(zones: &[u8]) -> Vec<(u8, u8)> {
        let mut out: Vec<(u8, u8)> = Vec::new();
        for &z in zones {
            match out.last_mut() {
                Some((_, last)) if *last as u16 + 1 == z as u16 => *last = z,
                _ => out.push((z, z)),
            }
        }
        out
    }

    /// Paints every zone one color.
    pub fn fill(dev: &mut Device, zones: &[u8], rgb: [u8; 3]) -> Result<()> {
        let ranges: Vec<_> = runs(zones).into_iter().map(|(a, b)| (a, b, rgb)).collect();
        set_ranges(dev, &ranges)
    }

    pub fn commit(dev: &mut Device) -> Result<()> {
        dev.call(PER_KEY_LIGHTING_V2, 7, &[0x00])?;
        Ok(())
    }
}

/// Starts a firmware effect by its effect id on `cluster` (0xff = all zones), with up to
/// 10 parameter bytes; the effect's index is looked up per device.
pub fn set_cluster_effect(
    dev: &mut Device,
    cluster: u8,
    effect_id: u16,
    params: &[u8],
) -> Result<()> {
    claim_lighting(dev)?;
    let index = cluster_effects(dev, cluster)?
        .into_iter()
        .find(|(_, id)| *id == effect_id)
        .map(|(i, _)| i)
        .ok_or(crate::Error::UnsupportedFeature(RGB_EFFECTS))?;
    let mut p = [0u8; 13];
    p[0] = cluster;
    p[1] = index;
    let n = params.len().min(10);
    p[2..2 + n].copy_from_slice(&params[..n]);
    p[12] = 0x01;
    dev.call(RGB_EFFECTS, 1, &p)?;
    Ok(())
}

/// Mouse sensor and report rate, decoded from G HUB captures of the G502 X PLUS.
/// Mouse button spy (0x8110): while started, the mouse reports every button state change as
/// event 0 with a 16-bit pressed mask, so buttons the host handles (DPI up/down/shift in host
/// mode) can act. Sequence from a G HUB capture: fn1 start, fn2 stop.
pub mod buttons {
    use crate::{Device, Result};

    pub const SPY: u16 = 0x8110;

    /// Starts reporting; returns the feature index events arrive on.
    pub fn start_spy(dev: &mut Device) -> Result<u8> {
        dev.call(SPY, 1, &[])?;
        dev.feature(SPY)
    }

    /// Pressed-button mask from a spy event, if `msg` is one. Button n is bit n−1.
    pub fn pressed(spy_index: u8, msg: &[u8]) -> Option<u16> {
        (msg[2] == spy_index && msg[3] >> 4 == 0).then(|| u16::from_be_bytes([msg[4], msg[5]]))
    }

    /// Number of buttons the spy covers (fn0; 11 on the G502 X PLUS).
    pub fn count(dev: &mut Device) -> Result<u8> {
        Ok(dev.call(SPY, 0, &[])?[4])
    }

    /// What each button sends to the PC: entry n−1 is the HID button that physical button n
    /// reports, 0 = nothing (the host handles it). Fn3; G HUB writes it with fn4.
    pub fn remapping(dev: &mut Device) -> Result<Vec<u8>> {
        let n = count(dev)?.min(16) as usize;
        Ok(dev.call(SPY, 3, &[])?[4..4 + n].to_vec())
    }

    pub fn set_remapping(dev: &mut Device, map: &[u8]) -> Result<()> {
        dev.call(SPY, 4, &map[..map.len().min(16)])?;
        Ok(())
    }
}

pub mod mouse {
    use crate::features::ADJUSTABLE_DPI;
    use crate::{Device, Result};

    pub const REPORT_RATE: u16 = 0x8060;

    /// Current DPI of sensor 0 (function 2 → [sensor, dpi_hi, dpi_lo, default_hi, default_lo]).
    pub fn dpi(dev: &mut Device) -> Result<u16> {
        let r = dev.call(ADJUSTABLE_DPI, 2, &[0x00])?;
        Ok(u16::from_be_bytes([r[5], r[6]]))
    }

    /// Supported DPI values (function 1). The list holds plain values and ranges written as
    /// `low, 0xe000 | step, high`.
    pub fn dpi_range(dev: &mut Device) -> Result<Vec<u16>> {
        let mut words = Vec::new();
        for page in 0..4u8 {
            let r = dev.call(ADJUSTABLE_DPI, 1, &[0x00, page])?;
            let chunk: Vec<u16> = r[5..20]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            let end = chunk.iter().position(|&w| w == 0);
            words.extend_from_slice(&chunk[..end.unwrap_or(chunk.len())]);
            if end.is_some() {
                break;
            }
        }
        Ok(words)
    }

    /// Sets sensor 0 to `dpi`. `level` is the DPI slot G HUB reports alongside (1-based;
    /// the mouse uses it for its DPI indicator).
    pub fn set_dpi(dev: &mut Device, dpi: u16, level: u8) -> Result<()> {
        let [hi, lo] = dpi.to_be_bytes();
        dev.call(ADJUSTABLE_DPI, 3, &[0x00, hi, lo, level])?;
        Ok(())
    }

    /// Report interval in ms (1 = 1000 Hz, 8 = 125 Hz).
    pub fn report_interval(dev: &mut Device) -> Result<u8> {
        Ok(dev.call(REPORT_RATE, 1, &[])?[4])
    }

    /// Supported intervals: bit n set = (n + 1) ms is allowed.
    pub fn report_intervals(dev: &mut Device) -> Result<Vec<u8>> {
        let mask = dev.call(REPORT_RATE, 0, &[])?[4];
        Ok((0..8)
            .filter(|b| mask & (1 << b) != 0)
            .map(|b| b + 1)
            .collect())
    }

    pub fn set_report_interval(dev: &mut Device, ms: u8) -> Result<()> {
        dev.call(REPORT_RATE, 2, &[ms])?;
        Ok(())
    }
}

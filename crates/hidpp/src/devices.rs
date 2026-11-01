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
    if has_feature(dev, PROFILE_MANAGEMENT) {
        dev.call(PROFILE_MANAGEMENT, 6, &[0x05])?;
    } else if has_feature(dev, ONBOARD_PROFILES) {
        dev.call(ONBOARD_PROFILES, 1, &[0x02])?;
    } else {
        return Ok(());
    }
    dev.call(RGB_EFFECTS, 5, &[0x01, 0x03, 0x04])?;
    Ok(())
}

/// Hands lighting (and onboard profiles) back to the device firmware.
pub fn release_lighting(dev: &mut Device) -> Result<()> {
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

/// Brightness (0x8040): function 2 sets the level, function 4 with 1 is always sent after it by G HUB.
pub fn set_brightness(dev: &mut Device, percent: u8) -> Result<()> {
    let level = (percent.min(100) as u16 * 255 / 100) as u8;
    dev.call(BRIGHTNESS, 2, &[0x00, level])?;
    dev.call(BRIGHTNESS, 4, &[0x01])?;
    Ok(())
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

    pub fn set_muted(dev: &mut Device, muted: bool) -> Result<()> {
        dev.call(MICROPHONE, 3, &[0x00, 0x00, if muted { 0 } else { 1 }])?;
        Ok(())
    }
}

/// Unified battery (0x1004) function 1: [state of charge %, level flags, charging status, ...].
pub fn battery(dev: &mut Device) -> Result<(u8, bool)> {
    let r = dev.call(crate::features::UNIFIED_BATTERY, 1, &[])?;
    Ok((r[4], matches!(r[6], 1..=3)))
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
    let count = dev.call(RGB_EFFECTS, 0, &[cluster, 0xff, 0x00])?[8];
    (0..count)
        .map(|i| {
            let r = dev.call(RGB_EFFECTS, 0, &[cluster, i, 0x00])?;
            Ok((i, u16::from_be_bytes([r[6], r[7]])))
        })
        .collect()
}

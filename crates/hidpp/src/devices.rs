//! Device-level operations decoded from G HUB USB captures.

use crate::features::{BRIGHTNESS, MICROPHONE, RGB_EFFECTS};
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

/// RGB effects (0x8071) function 1 on cluster 0. The trailing 0x01 makes the device keep it.
pub fn set_effect(dev: &mut Device, effect: Effect) -> Result<()> {
    let mut p = [0u8; 13];
    match effect {
        Effect::Solid { rgb } => {
            p[1] = 0;
            p[2..5].copy_from_slice(&rgb);
            p[5] = 0x02;
        }
        Effect::Breathe {
            rgb,
            period_ms,
            intensity,
        } => {
            p[1] = 1;
            p[2..5].copy_from_slice(&rgb);
            p[5..7].copy_from_slice(&period_ms.to_be_bytes());
            p[8] = intensity;
        }
        Effect::Cycle {
            period_ms,
            intensity,
            saturation,
        } => {
            p[1] = 2;
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

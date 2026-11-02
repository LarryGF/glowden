//! Battery life estimate from the device's power model (`battery.xml` in the resources
//! imported from G HUB): capacity, controller and report-rate draw, and lighting draw per
//! effect and brightness.

use crate::config::{DeviceConfig, Lighting};

fn attr(tag: &str, name: &str) -> Option<f32> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    tag[start..].split('"').next()?.parse().ok()
}

/// The first opening tag `<name ...>` at or after `from`, and where it ends.
fn tag<'a>(xml: &'a str, name: &str, from: usize) -> Option<(&'a str, usize)> {
    let start = from + xml[from..].find(&format!("<{name}"))?;
    let end = start + xml[start..].find('>')?;
    Some((&xml[start..=end], end))
}

/// Power (mW) of a lighting effect at `brightness`, from the nearest listed level.
fn lighting_power(xml: &str, effect: &str, brightness: u8) -> Option<f32> {
    let marker = format!("<effect type=\"{effect}\"");
    let start = xml.find(&marker)?;
    let head_end = start + xml[start..].find('>')?;
    if let Some(p) = attr(&xml[start..=head_end], "power") {
        return Some(p);
    }
    let body = &xml[head_end..head_end + xml[head_end..].find("</effect>")?];
    body.split("<brightness")
        .skip(1)
        .filter_map(|b| Some((attr(b, "percent")?, attr(b, "power")?)))
        .min_by(|a, b| {
            (a.0 - brightness as f32)
                .abs()
                .total_cmp(&(b.0 - brightness as f32).abs())
        })
        .map(|(_, p)| p)
}

/// Hours left at the current settings, or None without a power model for `model`.
pub fn hours_left(model: &str, percent: u8, cfg: &DeviceConfig) -> Option<f32> {
    let path = crate::assets::current().device_file(model, "battery.xml")?;
    let xml = std::fs::read_to_string(path).ok()?;
    let capacity_mwh = attr(tag(&xml, "capacity", 0)?.0, "watts_per_hour")? * 1000.0;
    let mcu = attr(tag(&xml, "mcu", 0)?.0, "power").unwrap_or(0.0);

    let interval = cfg.report_interval.unwrap_or(1) as f32;
    let mut report = 0.0;
    let mut pos = 0;
    while let Some((t, end)) = tag(&xml, "interval", pos) {
        if attr(t, "milliseconds") == Some(interval) {
            report = attr(t, "power").unwrap_or(0.0);
            break;
        }
        pos = end;
    }

    let off = cfg.power == Some(false);
    let effect = match &cfg.lighting {
        _ if off => "off",
        Some(Lighting::Breathing { .. }) => "breathing",
        Some(Lighting::Cycle { .. }) => "cycling_s",
        // Everything else keeps LEDs lit most of the time; "solid" is the closest model.
        _ => "solid",
    };
    let brightness = cfg.brightness.unwrap_or(100);
    let lighting = lighting_power(&xml, effect, brightness).unwrap_or(0.0);
    let load = if off {
        0.0
    } else {
        tag(&xml, "lighting", 0)
            .and_then(|(t, _)| attr(t, "mcu_load"))
            .unwrap_or(0.0)
    };

    let draw = mcu + report + lighting + load;
    (draw > 0.0).then(|| capacity_mwh * percent as f32 / 100.0 / draw)
}

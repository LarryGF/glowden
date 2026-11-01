use std::process::ExitCode;

use hidpp::devices::{self, Effect, mic};
use hidpp::features;

const USAGE: &str = "usage:
  lhub list                                  devices and their HID++ features
  lhub <dev> color RRGGBB
  lhub <dev> breathe RRGGBB [period_ms] [intensity%]
  lhub <dev> cycle [period_ms] [intensity%] [saturation]
  lhub <dev> brightness <0-100>
  lhub <dev> gain [0-100]                    microphones
  lhub <dev> mute | unmute
<dev> is a case-insensitive part of the device name, e.g. yeti, g515, g502";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => list(),
        Some(dev) if args.len() >= 2 => device_command(dev, &args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

type CmdResult = Result<(), Box<dyn std::error::Error>>;

fn list() -> CmdResult {
    for hidpp::discover::Found {
        device: mut dev,
        name,
        ..
    } in hidpp::discover::open_all()
    {
        let (major, minor) = match dev.protocol() {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "{name}  [{}: protocol query failed: {e}]",
                    dev.info.hidraw.display()
                );
                continue;
            }
        };
        println!(
            "{name}  [{:04x}:{:04x} {}{}{} HID++ {major}.{minor}]",
            dev.info.vendor,
            dev.info.product,
            dev.info.hidraw.display(),
            if dev.info.wireless { " wireless" } else { "" },
            if dev.index() != 0xff && !dev.info.wireless {
                format!(" slot {}", dev.index())
            } else {
                String::new()
            }
        );
        match dev.features() {
            Ok(list) => {
                for (id, version) in list {
                    let name = if dev.is_centurion() {
                        features::centurion_name(id)
                    } else {
                        features::name(id)
                    };
                    println!("    0x{id:04x} v{version}  {}", name.unwrap_or("?"));
                }
            }
            Err(e) => println!("    feature list failed: {e}"),
        }
    }
    Ok(())
}

fn device_command(query: &str, args: &[String]) -> CmdResult {
    let q = query.to_lowercase();
    let hidpp::discover::Found {
        device: mut dev,
        name,
        ..
    } = hidpp::discover::open_all()
        .into_iter()
        .find(|f| {
            f.name
                .to_lowercase()
                .replace(' ', "")
                .contains(&q.replace(' ', ""))
        })
        .ok_or_else(|| format!("no device matching '{query}'"))?;
    let arg = |i: usize| args.get(i).map(String::as_str);
    let num = |i: usize, default: u32, max: u32| -> Result<u32, String> {
        let v = arg(i).map_or(Ok(default), |s| {
            s.parse().map_err(|_| format!("bad number '{s}'"))
        })?;
        if v > max {
            Err(format!("{v} is above {max}"))
        } else {
            Ok(v)
        }
    };
    match (arg(0).unwrap_or(""), args.len()) {
        ("color", 2) => devices::set_effect(
            &mut dev,
            Effect::Solid {
                rgb: rgb(&args[1])?,
            },
        )?,
        ("breathe", 2..=4) => devices::set_effect(
            &mut dev,
            Effect::Breathe {
                rgb: rgb(&args[1])?,
                period_ms: num(2, 5000, 20000)? as u16,
                intensity: num(3, 100, 100)? as u8,
            },
        )?,
        ("cycle", 1..=4) => devices::set_effect(
            &mut dev,
            Effect::Cycle {
                period_ms: num(1, 5000, 20000)? as u16,
                intensity: num(2, 100, 100)? as u8,
                saturation: num(3, 255, 255)? as u8,
            },
        )?,
        ("brightness", 2) => devices::set_brightness(&mut dev, num(1, 0, 100)? as u8)?,
        ("gain", 1) => println!("{name}: gain {}", mic::gain(&mut dev)?),
        ("gain", 2) => mic::set_gain(&mut dev, num(1, 0, 100)? as u8)?,
        ("mute", 1) => mic::set_muted(&mut dev, true)?,
        ("unmute", 1) => mic::set_muted(&mut dev, false)?,
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn rgb(s: &str) -> Result<[u8; 3], String> {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16)
        .ok()
        .filter(|_| s.len() == 6)
        .ok_or("color must be RRGGBB")?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

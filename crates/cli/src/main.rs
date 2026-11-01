use std::process::ExitCode;

use hidpp::devices::{self, Effect, mic};
use hidpp::features;

const USAGE: &str = "usage:
  lhub list                                  devices and their HID++ features
  lhub import-ghub <dir>                     copy device pictures/layouts from a G HUB install
  lhub <dev> color RRGGBB
  lhub <dev> breathe RRGGBB [period_ms] [intensity%]
  lhub <dev> cycle [period_ms] [intensity%] [saturation]
  lhub <dev> brightness <0-100>
  lhub <dev> release                         hand lighting/profiles back to the device
  lhub <dev> animate wave|pulse|starlight    host-rendered per-key effect (Ctrl+C stops)
  lhub <dev> gain [0-100]                    microphones
  lhub <dev> mute | unmute
<dev> is a case-insensitive part of the device name, e.g. yeti, g515, g502";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => list(),
        Some("audio-levels") => audio_levels(),
        Some("import-ghub") if args.len() == 2 => import_ghub(std::path::Path::new(&args[1])),
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
        ("release", 1) => devices::release_lighting(&mut dev)?,
        ("brightness-info", 1) => {
            let r = dev.call(hidpp::features::BRIGHTNESS, 0, &[])?;
            let cur = dev.call(hidpp::features::BRIGHTNESS, 1, &[])?;
            println!(
                "{name}: info {:02x?} current {:02x?}",
                &r[4..12],
                &cur[4..8]
            );
        }
        ("mouse", 1) => {
            use devices::mouse;
            println!(
                "{name}: dpi {} (supported {:?})",
                mouse::dpi(&mut dev)?,
                mouse::dpi_range(&mut dev)?
            );
            println!(
                "{name}: report interval {} ms (supported {:?} ms)",
                mouse::report_interval(&mut dev)?,
                mouse::report_intervals(&mut dev)?
            );
        }
        ("dpi", 2) => devices::mouse::set_dpi(&mut dev, args[1].parse()?, 1)?,
        ("report-rate", 2) => devices::mouse::set_report_interval(&mut dev, args[1].parse()?)?,
        ("raw-effect", 4) => {
            // Experiments: cluster, effect index, up to 10 hex parameter bytes.
            let cluster = u8::from_str_radix(&args[1], 16)?;
            let index = u8::from_str_radix(&args[2], 16)?;
            let bytes: Vec<u8> = (0..args[3].len() / 2)
                .map(|i| u8::from_str_radix(&args[3][i * 2..i * 2 + 2], 16))
                .collect::<Result<_, _>>()?;
            let mut p = [0u8; 13];
            p[0] = cluster;
            p[1] = index;
            p[2..2 + bytes.len().min(10)].copy_from_slice(&bytes[..bytes.len().min(10)]);
            p[12] = 0x01;
            devices::claim_lighting(&mut dev)?;
            dev.call(hidpp::features::RGB_EFFECTS, 1, &p)?;
        }
        ("animate", 2) => animate(&mut dev, &name, &args[1])?,
        ("zones", 1) => {
            let z = devices::perkey::zones(&mut dev)?;
            println!(
                "{name}: {} zones: {}",
                z.len(),
                z.iter()
                    .map(|z| format!("{z:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        ("fill", 2) => {
            let c = rgb(&args[1])?;
            let zones = devices::perkey::zones(&mut dev)?;
            devices::perkey::begin(&mut dev)?;
            devices::perkey::fill(&mut dev, &zones, c)?;
            devices::perkey::commit(&mut dev)?;
        }
        ("key", 3) => {
            let zone = u8::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
            let zones = devices::perkey::zones(&mut dev)?;
            devices::perkey::begin(&mut dev)?;
            devices::perkey::fill(&mut dev, &zones, [0, 0, 0])?;
            devices::perkey::set_keys(&mut dev, &[(zone, rgb(&args[2])?)])?;
            devices::perkey::commit(&mut dev)?;
        }
        ("effects", 1) => {
            for cluster in [0x00, 0xff] {
                match devices::cluster_effects(&mut dev, cluster) {
                    Ok(list) => {
                        for (index, id) in list {
                            println!(
                                "{name}: cluster 0x{cluster:02x} effect {index} = id 0x{id:04x}"
                            );
                        }
                    }
                    Err(e) => println!("{name}: cluster 0x{cluster:02x}: {e}"),
                }
            }
        }
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

fn animate(dev: &mut hidpp::Device, name: &str, which: &str) -> CmdResult {
    use lighting::effects::{Pulse, Starlight, Wave};
    use lighting::engine::{Effect, Press, Streamer};
    use std::time::{Duration, Instant};

    let mut effect: Box<dyn Effect> = match which {
        "wave" => Box::new(Wave {
            period: 4.0,
            spread: 1.0,
        }),
        "pulse" => Box::new(Pulse::new([0x2d, 0xd4, 0xbf], [0x02, 0x06, 0x0c])),
        "starlight" => Box::new(Starlight::new([0xff, 0xf2, 0xd0], [0x00, 0x03, 0x10])),
        _ => return Err(USAGE.into()),
    };
    let short = name.split_whitespace().next().unwrap_or(name);
    let nodes = lighting::input::find_event_nodes(short);
    let presses_rx = lighting::input::watch(&nodes)?;
    eprintln!("animating {which} on {name}; key input from {nodes:?}; Ctrl+C to stop");

    let mut streamer = Streamer::new(dev, lighting::layout::iso_tkl_positions())?;
    let mut presses: Vec<Press> = Vec::new();
    let (mut frames, mut sent, mut since) = (0u32, 0usize, Instant::now());
    loop {
        let start = Instant::now();
        presses.extend(presses_rx.try_iter());
        presses.retain(|p| p.at.elapsed() < Duration::from_secs(3));
        sent += streamer.frame(dev, effect.as_mut(), &presses)?;
        frames += 1;
        if since.elapsed() >= Duration::from_secs(5) {
            let secs = since.elapsed().as_secs_f32();
            eprintln!(
                "{:.1} fps, {:.0} key updates/s",
                frames as f32 / secs,
                sent as f32 / secs
            );
            (frames, sent, since) = (0, 0, Instant::now());
        }
        if let Some(rest) = streamer.frame_interval.checked_sub(start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// Prints the visualizer's band levels for a few seconds (checks audio capture).
fn audio_levels() -> CmdResult {
    let meter = lighting::audio::AudioMeter::start(16)?;
    for _ in 0..12 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let bars: String = meter
            .bands
            .lock()
            .unwrap()
            .iter()
            .map(|v| [' ', '.', ':', '-', '=', '+', '*', '#', '@'][(v * 8.0).round() as usize])
            .collect();
        println!("[{bars}]");
    }
    Ok(())
}

/// Copies device resource packages (pictures, image geometry, light maps, battery curves)
/// out of a G HUB `depots` folder into ~/.local/share/lhub/devices/<model>/. They stay on
/// this machine; nothing from G HUB is shipped with lhub.
fn import_ghub(root: &std::path::Path) -> CmdResult {
    use std::path::{Path, PathBuf};
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "manifest.json") {
                out.push(p);
            }
        }
    }
    let wanted = |p: &Path| {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let in_images = p.components().any(|c| c.as_os_str() == "images");
        matches!(ext, "webp" | "png" | "xml")
            || (ext.is_empty() && in_images)
            || matches!(
                name,
                "metadata.json"
                    | "light_map.json"
                    | "lighting_effects.json"
                    | "default_configurations.json"
                    | "defaults_override.json"
            )
    };
    let dest_root = PathBuf::from(std::env::var("HOME")?).join(".local/share/lhub/devices");
    let mut manifests = Vec::new();
    walk(root, &mut manifests);
    let models = ["g515_ls_tkl", "g502x_plus", "g522", "yeti_gx"];
    for manifest in manifests {
        let text = std::fs::read_to_string(&manifest).unwrap_or_default();
        let dir = manifest.parent().unwrap();
        // Device resource packages have a metadata.json describing the device image.
        if !dir.join("metadata.json").exists() {
            continue;
        }
        let Some(model) = models.iter().find(|m| text.contains(&format!("\"{m}\""))) else {
            continue;
        };
        let dest = dest_root.join(model);
        let mut files = Vec::new();
        fn collect(
            dir: &Path,
            base: &Path,
            wanted: &dyn Fn(&Path) -> bool,
            out: &mut Vec<(PathBuf, PathBuf)>,
        ) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, base, wanted, out);
                } else if wanted(&p) {
                    out.push((p.clone(), p.strip_prefix(base).unwrap().to_path_buf()));
                }
            }
        }
        collect(dir, dir, &wanted, &mut files);
        for (src, rel) in &files {
            // Some packages store pictures without an extension; name them by their content.
            // Encrypted assets (G HUB fetches their keys online) are left out.
            let mut rel = rel.clone();
            if rel.extension().is_none() {
                let head = std::fs::read(src)
                    .map(|b| b[..b.len().min(12)].to_vec())
                    .unwrap_or_default();
                if head.starts_with(b"\x89PNG") {
                    rel.set_extension("png");
                } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
                    rel.set_extension("webp");
                } else {
                    continue;
                }
            }
            let to = dest.join(&rel);
            std::fs::create_dir_all(to.parent().unwrap())?;
            std::fs::copy(src, &to)?;
        }
        println!("{model}: {} files -> {}", files.len(), dest.display());
    }
    Ok(())
}

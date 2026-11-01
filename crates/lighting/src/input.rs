//! Key presses from a keyboard's evdev node, mapped to per-key zones.

use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use crate::engine::Press;
use crate::layout;

const EV_KEY: u16 = 1;
/// struct input_event on 64-bit Linux: timeval (16) + type (2) + code (2) + value (4).
const EVENT_SIZE: usize = 24;

/// Evdev nodes whose device name contains `name` (case-insensitive).
pub fn find_event_nodes(name: &str) -> Vec<PathBuf> {
    let want = name.to_lowercase();
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/input") else {
        return out;
    };
    for e in entries.flatten() {
        let file = e.file_name();
        let node = file.to_string_lossy();
        if !node.starts_with("event") {
            continue;
        }
        let dev_name = fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
        if dev_name.to_lowercase().contains(&want) {
            out.push(PathBuf::from("/dev/input").join(&*node));
        }
    }
    out.sort();
    out
}

/// Spawns a reader thread per node; key-down events arrive as presses on the channel.
pub fn watch(nodes: &[PathBuf]) -> std::io::Result<Receiver<Press>> {
    let (tx, rx) = mpsc::channel();
    for node in nodes {
        let mut file = File::open(node)?;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; EVENT_SIZE];
            while file.read_exact(&mut buf).is_ok() {
                let kind = u16::from_ne_bytes([buf[16], buf[17]]);
                let code = u16::from_ne_bytes([buf[18], buf[19]]);
                let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
                if kind != EV_KEY || value != 1 {
                    continue;
                }
                let zone = layout::usage_for_keycode(code).and_then(layout::zone_for_usage);
                if let Some(zone) = zone
                    && tx
                        .send(Press {
                            zone,
                            at: Instant::now(),
                        })
                        .is_err()
                {
                    break;
                }
            }
        });
    }
    Ok(rx)
}

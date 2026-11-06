//! Performing an assigned action (mouse buttons, remapped keys) through the virtual input
//! device.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Duration;

use crate::config::{Action, MacroStep};

/// `holding` keeps the keys a held button or key pressed, so they're released with it.
pub fn perform<K: Hash + Eq>(
    holding: &mut HashMap<K, Vec<u16>>,
    n: K,
    action: &Action,
    down: bool,
) -> Result<(), String> {
    let codes = |names: &[String]| -> Result<Vec<u16>, String> {
        names
            .iter()
            .map(|k| crate::keys::code(k).ok_or_else(|| format!("unknown key {k}")))
            .collect()
    };
    match action {
        Action::Keys { keys } => {
            let input = crate::uinput::shared()?;
            let mut input = input.lock().unwrap();
            if down {
                let codes = codes(keys)?;
                for &c in &codes {
                    input.key(c, true).map_err(|e| e.to_string())?;
                }
                holding.insert(n, codes);
            } else if let Some(codes) = holding.remove(&n) {
                for &c in codes.iter().rev() {
                    input.key(c, false).map_err(|e| e.to_string())?;
                }
            }
        }
        Action::MouseButton { button } => {
            let code = codes(std::slice::from_ref(button))?[0];
            let input = crate::uinput::shared()?;
            input
                .lock()
                .unwrap()
                .key(code, down)
                .map_err(|e| e.to_string())?;
        }
        Action::Scroll { direction } if down => {
            let (axis, notches) = match direction.as_str() {
                "up" => (crate::uinput::REL_WHEEL, 1),
                "down" => (crate::uinput::REL_WHEEL, -1),
                "left" => (crate::uinput::REL_HWHEEL, -1),
                "right" => (crate::uinput::REL_HWHEEL, 1),
                other => return Err(format!("unknown scroll direction {other}")),
            };
            let input = crate::uinput::shared()?;
            input
                .lock()
                .unwrap()
                .scroll(axis, notches)
                .map_err(|e| e.to_string())?;
        }
        Action::Command { command } if down => {
            std::process::Command::new("sh")
                .args(["-c", command])
                .stdin(std::process::Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Action::Sound { file } if down => {
            std::process::Command::new("pw-play")
                .arg(file)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("cannot play {file}: {e}"))?;
        }
        Action::Macro { steps } if down => {
            // Played on its own thread so pauses don't hold up the device.
            let input = crate::uinput::shared()?;
            let steps: Vec<Result<Vec<u16>, u32>> = steps
                .iter()
                .map(|s| match s {
                    MacroStep::Keys { keys } => codes(keys).map(Ok),
                    MacroStep::Pause { ms } => Ok(Err(*ms)),
                })
                .collect::<Result<_, _>>()?;
            std::thread::spawn(move || {
                for step in steps {
                    match step {
                        Ok(codes) => {
                            let mut input = input.lock().unwrap();
                            for &c in &codes {
                                let _ = input.key(c, true);
                            }
                            for &c in codes.iter().rev() {
                                let _ = input.key(c, false);
                            }
                        }
                        Err(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
                    }
                }
            });
        }
        _ => {}
    }
    Ok(())
}

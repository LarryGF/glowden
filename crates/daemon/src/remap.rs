//! Keyboard key remapping on the host. While a keyboard has remaps, its typing interface
//! (the evdev node with letter keys) is grabbed: remapped keys perform their action, every
//! other key is passed on unchanged through the virtual input device. Nodes without letter
//! keys (a separate media-key interface, where a keyboard has one) aren't grabbed.
//!
//! Safety: Left Ctrl + Right Ctrl + Esc releases the keyboard at once (until the remaps
//! change or the service restarts), the kernel drops the grab when the service exits or
//! crashes, and keys held through the virtual device are released when a grab ends.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::config::Action;

const EV_KEY: u16 = 1;
const KEY_ESC: u16 = 1;
const KEY_LEFTCTRL: u16 = 29;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_A: u16 = 30;
/// _IOW('E', 0x90, int) from <linux/input.h>.
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
/// struct input_event on 64-bit Linux: timeval (16) + type (2) + code (2) + value (4).
const EVENT_SIZE: usize = 24;

type Table = Arc<Mutex<HashMap<u16, Action>>>;

/// A device worker's remapping: started, updated or stopped as its config changes.
#[derive(Default)]
pub struct Keys {
    running: Option<Remapper>,
    /// The remaps last applied, so an escaped grab stays released until they change.
    applied: BTreeMap<String, Action>,
}

impl Keys {
    pub fn configure(&mut self, id: &str, name: &str, remaps: &BTreeMap<String, Action>) {
        if *remaps == self.applied {
            return;
        }
        self.applied = remaps.clone();
        let table: HashMap<u16, Action> = remaps
            .iter()
            .filter_map(|(k, a)| match crate::keys::code(k) {
                Some(c) => Some((c, a.clone())),
                None => {
                    eprintln!("{id}: key remap: unknown key {k}");
                    None
                }
            })
            .collect();
        if table.is_empty() {
            if self.running.take().is_some() {
                eprintln!("{id}: key remapping off, keyboard released");
            }
            return;
        }
        if let Some(r) = self.running.as_ref().filter(|r| !r.ended()) {
            *r.table.lock().unwrap() = table;
            return;
        }
        self.running = None;
        match Remapper::start(id, name, table) {
            Ok(r) => {
                eprintln!("{id}: key remapping on ({} keys)", remaps.len());
                self.running = Some(r);
            }
            Err(e) => eprintln!("{id}: key remapping unavailable: {e}"),
        }
    }
}

struct Remapper {
    table: Table,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Remapper {
    fn start(id: &str, name: &str, table: HashMap<u16, Action>) -> Result<Remapper, String> {
        let nodes = typing_nodes(name);
        if nodes.is_empty() {
            return Err(format!("no keyboard input node named like {name}"));
        }
        let input = crate::uinput::shared()?;
        let table = Arc::new(Mutex::new(table));
        let stop = Arc::new(AtomicBool::new(false));
        let mut files = Vec::new();
        for node in &nodes {
            let file = File::open(node).map_err(|e| format!("{}: {e}", node.display()))?;
            if unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } < 0 {
                // Files opened so far close here, which releases their grabs.
                let e = std::io::Error::last_os_error();
                return Err(format!("cannot grab {}: {e}", node.display()));
            }
            files.push(file);
        }
        let threads = files
            .into_iter()
            .map(|file| {
                let (table, stop, input, id) =
                    (table.clone(), stop.clone(), input.clone(), id.to_string());
                std::thread::Builder::new()
                    .name(format!("remap {id}"))
                    .spawn(move || pass_through(file, &table, &stop, &input, &id))
                    .expect("spawn remap thread")
            })
            .collect();
        Ok(Remapper {
            table,
            stop,
            threads,
        })
    }

    /// Escaped, or the keyboard went away.
    fn ended(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.threads.iter().all(|t| t.is_finished())
    }
}

impl Drop for Remapper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// Reads one grabbed node until stopped; the grab ends when `file` closes on return.
fn pass_through(
    mut file: File,
    table: &Table,
    stop: &AtomicBool,
    input: &Mutex<crate::uinput::VirtualInput>,
    id: &str,
) {
    let mut physical: HashSet<u16> = HashSet::new();
    let mut forwarded: HashSet<u16> = HashSet::new();
    let mut holding: HashMap<u16, Vec<u16>> = HashMap::new();
    let mut buf = [0u8; EVENT_SIZE];
    let fd = file.as_raw_fd();
    while !stop.load(Ordering::Relaxed) {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // Wakes up regularly so a stop request is noticed.
        match unsafe { libc::poll(&mut pfd, 1, 200) } {
            0 => continue,
            n if n < 0 => break,
            _ => {}
        }
        if file.read_exact(&mut buf).is_err() {
            break; // unplugged
        }
        let kind = u16::from_ne_bytes([buf[16], buf[17]]);
        let code = u16::from_ne_bytes([buf[18], buf[19]]);
        let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
        if kind != EV_KEY {
            continue;
        }
        match value {
            1 => {
                physical.insert(code);
            }
            0 => {
                physical.remove(&code);
            }
            _ => {}
        }
        if value == 1
            && code == KEY_ESC
            && physical.contains(&KEY_LEFTCTRL)
            && physical.contains(&KEY_RIGHTCTRL)
        {
            stop.store(true, Ordering::Relaxed);
            eprintln!("{id}: key remapping stopped with Left Ctrl + Right Ctrl + Esc");
            crate::app::notify(
                "Keyboard remapping stopped",
                "The keyboard was released. Remapping resumes when you change a remap or restart the service.",
            );
            break;
        }
        let action = table.lock().unwrap().get(&code).cloned();
        match action {
            // Auto-repeat of a remapped key is dropped; its action ran on the press.
            Some(a) if value != 2 => {
                if let Err(e) = crate::actions::perform(&mut holding, code, &a, value == 1) {
                    eprintln!("{id}: key remap: {e}");
                }
            }
            Some(_) => {}
            None => {
                let _ = input.lock().unwrap().key_event(code, value);
                match value {
                    0 => {
                        forwarded.remove(&code);
                    }
                    1 => {
                        forwarded.insert(code);
                    }
                    _ => {}
                }
            }
        }
    }
    // Nothing stays pressed on the virtual device once the grab ends.
    let mut input = input.lock().unwrap();
    for code in forwarded.into_iter().chain(holding.into_values().flatten()) {
        let _ = input.key(code, false);
    }
}

/// The keyboard's evdev nodes that carry letter keys, by device name.
fn typing_nodes(name: &str) -> Vec<PathBuf> {
    lighting::input::find_event_nodes(name)
        .into_iter()
        .filter(|node| {
            let Some(event) = node.file_name() else {
                return false;
            };
            let caps = std::fs::read_to_string(
                PathBuf::from("/sys/class/input")
                    .join(event)
                    .join("device/capabilities/key"),
            )
            .unwrap_or_default();
            // Space-separated 64-bit words, most significant first; KEY_A is in the last.
            caps.split_whitespace()
                .last()
                .and_then(|w| u64::from_str_radix(w, 16).ok())
                .is_some_and(|w| w & (1 << KEY_A) != 0)
        })
        .collect()
}

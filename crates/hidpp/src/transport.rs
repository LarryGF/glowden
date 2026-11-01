use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::features;

const REPORT_SHORT: u8 = 0x10;
const REPORT_LONG: u8 = 0x11;
const SHORT_LEN: usize = 7;
const LONG_LEN: usize = 20;
/// Headset receivers (G522) wrap HID++ in 64-byte "centurion" frames on report 0x50.
const REPORT_CENTURION: u8 = 0x50;
const CENTURION_LEN: usize = 64;
/// Replies are normalised to the HID++ long layout: [report, index, feature, function, params...].
/// Large enough for centurion replies split across several frames.
pub const REPLY_LEN: usize = 256;
/// Software id in the low nibble of the function byte; lets us spot our own replies.
const SWID: u8 = 0x0A;
const TIMEOUT: Duration = Duration::from_millis(500);
const LOGITECH: u16 = 0x046d;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Timeout,
    /// HID++ 2.0 error reply: feature index, error code.
    Device(u8, u8),
    UnsupportedFeature(u16),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Timeout => write!(f, "device did not answer"),
            Error::Device(idx, code) => {
                write!(f, "device error 0x{code:02x} on feature index {idx}")
            }
            Error::UnsupportedFeature(id) => write!(f, "device lacks feature 0x{id:04x}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub hidraw: PathBuf,
    pub vendor: u16,
    pub product: u16,
    pub kernel_name: String,
    /// Receiver-attached devices sit under the receiver's HID++ interface.
    pub wireless: bool,
    /// Physical USB port (e.g. "3-4.2.2"); stable across replugs, unlike hidraw numbers.
    pub port: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Framing {
    HidPP {
        has_short: bool,
    },
    /// Address learned by probing; the dongle only answers the right one.
    Centurion {
        addr: u8,
    },
}

pub struct Device {
    pub info: DeviceInfo,
    file: File,
    index: u8,
    framing: Framing,
    /// Index of the dongle's bridge feature; when set, requests go to the device behind it.
    bridge: Option<u8>,
    feature_cache: HashMap<u16, u8>,
}

/// Dongle feature that forwards messages to the headset behind a centurion receiver.
const CENTURION_BRIDGE: u16 = 0x0003;

impl Device {
    pub fn open(info: DeviceInfo) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&info.hidraw)?;
        let desc =
            fs::read(info.hidraw_sysfs().join("device/report_descriptor")).unwrap_or_default();
        let index = if info.wireless { 0x01 } else { 0xff };
        let framing = if has_report(&desc, REPORT_CENTURION) {
            Framing::Centurion { addr: 0 }
        } else {
            Framing::HidPP {
                has_short: has_report(&desc, REPORT_SHORT),
            }
        };
        let mut dev = Device {
            info,
            file,
            index,
            framing,
            bridge: None,
            feature_cache: HashMap::new(),
        };
        if let Framing::Centurion { .. } = framing {
            dev.probe_centurion_addr()?;
            let r = dev.direct_request(0x00, 0, &CENTURION_BRIDGE.to_be_bytes())?;
            if r[4] != 0 {
                dev.bridge = Some(r[4]);
            }
        }
        Ok(dev)
    }

    fn probe_centurion_addr(&mut self) -> Result<()> {
        for addr in 0..=255u8 {
            self.framing = Framing::Centurion { addr };
            self.send(0x00, 1, &[0, 0, 0])?;
            if self.receive(0x00, 1, Duration::from_millis(5)).is_ok() {
                return Ok(());
            }
        }
        Err(Error::Timeout)
    }

    fn send(&mut self, feature_index: u8, function: u8, params: &[u8]) -> Result<()> {
        let mut pkt = [0u8; CENTURION_LEN];
        let len = match self.framing {
            Framing::Centurion { addr } => {
                pkt[0] = REPORT_CENTURION;
                pkt[1] = addr;
                pkt[2] = (params.len() + 3) as u8; // flags + feature + function + params
                pkt[4] = feature_index;
                pkt[5] = (function << 4) | SWID;
                pkt[6..6 + params.len()].copy_from_slice(params);
                CENTURION_LEN
            }
            Framing::HidPP { has_short } => {
                let long = params.len() > SHORT_LEN - 4 || !has_short;
                pkt[0] = if long { REPORT_LONG } else { REPORT_SHORT };
                pkt[1] = self.index;
                pkt[2] = feature_index;
                pkt[3] = (function << 4) | SWID;
                pkt[4..4 + params.len()].copy_from_slice(params);
                if long { LONG_LEN } else { SHORT_LEN }
            }
        };
        self.drain()?;
        if debug() {
            eprintln!("{} -> {}", self.info.hidraw.display(), hex(&pkt[..len]));
        }
        self.file.write_all(&pkt[..len])?;
        Ok(())
    }

    /// Drops input queued before a request, so a late reply to an earlier request can't be
    /// taken as the answer to this one.
    fn drain(&mut self) -> Result<()> {
        let mut buf = [0u8; CENTURION_LEN];
        while wait_readable(&self.file, Duration::ZERO)? {
            let n = self.file.read(&mut buf)?;
            if debug() {
                eprintln!(
                    "{} <- {} (stale, dropped)",
                    self.info.hidraw.display(),
                    hex(&buf[..n])
                );
            }
        }
        Ok(())
    }

    /// Reads until the reply to (feature_index, function) arrives.
    fn receive(
        &mut self,
        feature_index: u8,
        function: u8,
        timeout: Duration,
    ) -> Result<[u8; REPLY_LEN]> {
        let func_sw = (function << 4) | SWID;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Some(msg) = self.read_message(left)? else {
                return Err(Error::Timeout);
            };
            if matches!(self.framing, Framing::HidPP { .. }) && msg[1] != self.index {
                continue; // another device on the same receiver
            }
            if msg[2] == 0xff && msg[3] == feature_index && msg[4] == func_sw {
                return Err(Error::Device(feature_index, msg[5]));
            }
            if msg[2] == feature_index && msg[3] == func_sw {
                return Ok(msg);
            }
        }
    }

    /// Sends one request and waits for the matching reply. Behind a centurion bridge the
    /// request is addressed to the headset, not the dongle.
    pub fn request(
        &mut self,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<[u8; REPLY_LEN]> {
        // Dozing wireless devices drop single requests; resending wakes them.
        let attempts = if self.info.wireless || self.bridge.is_some() {
            3
        } else {
            1
        };
        let mut last = Err(Error::Timeout);
        for _ in 0..attempts {
            last = match self.bridge {
                Some(bridge) => self.bridged_request(bridge, feature_index, function, params),
                None => self.direct_request(feature_index, function, params),
            };
            if !matches!(last, Err(Error::Timeout)) {
                break;
            }
        }
        // 0x08 = busy: the device is still digesting earlier messages (e.g. lighting frames).
        let mut busy_retries = 0;
        while matches!(last, Err(Error::Device(_, 0x08))) && busy_retries < 20 {
            std::thread::sleep(Duration::from_millis(2 + busy_retries));
            busy_retries += 1;
            last = match self.bridge {
                Some(bridge) => self.bridged_request(bridge, feature_index, function, params),
                None => self.direct_request(feature_index, function, params),
            };
        }
        last
    }

    fn direct_request(
        &mut self,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<[u8; REPLY_LEN]> {
        self.send(feature_index, function, params)?;
        self.receive(feature_index, function, TIMEOUT)
    }

    /// Bridge frame: [len header (2), inner cpl, inner feature, inner function, params...].
    /// The dongle acks with our swid, then delivers the headset's reply as an event (swid 0):
    /// [bridge, 0x10, len (2), inner cpl, inner feature, inner function, data...].
    fn bridged_request(
        &mut self,
        bridge: u8,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<[u8; REPLY_LEN]> {
        let inner_len = 3 + params.len();
        let max = CENTURION_LEN - 6 - 4;
        if inner_len > max {
            return Err(Error::Io(std::io::Error::other(
                "bridged request too long for one frame",
            )));
        }
        let func_sw = (function << 4) | SWID;
        let mut body = vec![
            (inner_len >> 8) as u8 & 0x0f,
            inner_len as u8,
            0x00,
            feature_index,
            func_sw,
        ];
        body.extend_from_slice(params);
        self.send(bridge, 1, &body)?;

        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Some(m) = self.read_message(left)? else {
                return Err(Error::Timeout);
            };
            if m[2] != bridge || m[3] != 0x10 || m[6] == 0xff {
                continue; // our ack, other traffic, or an unsolicited notification
            }
            if m[7] == 0xff && m[8] == feature_index && m[9] == func_sw {
                return Err(Error::Device(feature_index, m[10]));
            }
            if m[7] == feature_index && m[8] == func_sw {
                let mut out = [0u8; REPLY_LEN];
                out[0] = REPORT_LONG;
                out[1] = 0xff;
                out[2..REPLY_LEN - 5].copy_from_slice(&m[7..]);
                return Ok(out);
            }
        }
    }

    /// Next HID++ message on this node, normalised to [report, index, feature, function, params...].
    /// Centurion frames carry flags = (fragment << 1) | more; fragments are joined into one message.
    fn read_message(&mut self, timeout: Duration) -> Result<Option<[u8; REPLY_LEN]>> {
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; CENTURION_LEN];
        let mut joined: Vec<u8> = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !wait_readable(&self.file, left)? {
                return Ok(None);
            }
            let n = self.file.read(&mut buf)?;
            if debug() {
                eprintln!("{} <- {}", self.info.hidraw.display(), hex(&buf[..n]));
            }
            let mut msg = [0u8; REPLY_LEN];
            match (buf[0], self.framing) {
                (REPORT_CENTURION, Framing::Centurion { addr }) if n >= 5 && buf[1] == addr => {
                    let end = (3 + buf[2] as usize).min(n);
                    let (fragment, more) = (buf[3] >> 1, buf[3] & 1 != 0);
                    if fragment == 0 {
                        joined.clear();
                    } else if joined.is_empty() {
                        continue; // continuation of a message we didn't see start
                    }
                    joined.extend_from_slice(&buf[4..end.max(4)]);
                    if more {
                        continue;
                    }
                    msg[0] = REPORT_LONG;
                    msg[1] = 0xff;
                    let len = joined.len().min(REPLY_LEN - 2);
                    msg[2..2 + len].copy_from_slice(&joined[..len]);
                    joined.clear();
                }
                (REPORT_SHORT | REPORT_LONG, Framing::HidPP { .. }) if n >= 5 => {
                    msg[..n].copy_from_slice(&buf[..n])
                }
                _ => continue,
            }
            return Ok(Some(msg));
        }
    }

    pub fn is_centurion(&self) -> bool {
        matches!(self.framing, Framing::Centurion { .. })
    }

    /// Resolves a feature id to this device's index via the ROOT feature.
    pub fn feature(&mut self, id: u16) -> Result<u8> {
        if let Some(&idx) = self.feature_cache.get(&id) {
            return Ok(idx);
        }
        let r = self.request(0x00, 0, &id.to_be_bytes())?;
        if r[4] == 0 && id != features::ROOT {
            return Err(Error::UnsupportedFeature(id));
        }
        self.feature_cache.insert(id, r[4]);
        Ok(r[4])
    }

    pub fn call(
        &mut self,
        feature_id: u16,
        function: u8,
        params: &[u8],
    ) -> Result<[u8; REPLY_LEN]> {
        let idx = self.feature(feature_id)?;
        self.request(idx, function, params)
    }

    /// Device index on the node: 0xff for directly attached devices, 1..=6 for receiver slots.
    pub fn index(&self) -> u8 {
        self.index
    }

    /// Opens the device paired in `slot` behind a receiver that has no per-device node.
    pub fn open_slot(info: DeviceInfo, slot: u8) -> Result<Self> {
        let mut dev = Device::open(info)?;
        dev.index = slot;
        Ok(dev)
    }

    /// HID++ protocol version as (major, minor); fails for devices that don't speak HID++ 2.0.
    pub fn protocol(&mut self) -> Result<(u8, u8)> {
        let r = self.request(0x00, 1, &[0, 0, 0x5a])?;
        Ok((r[4], r[5]))
    }

    /// Name reported by the device. Centurion headsets use 0x0005 for something else, so
    /// they get the USB product name the kernel reports.
    pub fn name(&mut self) -> Result<String> {
        if self.is_centurion() {
            return Ok(self
                .info
                .kernel_name
                .trim_start_matches("Logitech ")
                .to_string());
        }
        let len = self.call(features::DEVICE_NAME, 0, &[])?[4] as usize;
        let mut name = Vec::with_capacity(len);
        while name.len() < len {
            let r = self.call(features::DEVICE_NAME, 1, &[name.len() as u8])?;
            let take = (len - name.len()).min(16);
            name.extend_from_slice(&r[4..4 + take]);
        }
        Ok(String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .to_string())
    }

    /// All (feature id, version) pairs the device reports, in index order.
    pub fn features(&mut self) -> Result<Vec<(u16, u8)>> {
        let count = self.call(features::FEATURE_SET, 0, &[])?[4];
        if self.bridge.is_some() {
            // Behind the bridge, function 1 returns [remaining, then 4-byte entries from the
            // requested index onward] in one (fragmented) reply.
            let r = self.call(features::FEATURE_SET, 1, &[1])?;
            let entries = (r[4] as usize).min(count as usize);
            return Ok((0..entries)
                .map(|i| &r[5 + i * 4..9 + i * 4])
                .map(|e| (u16::from_be_bytes([e[0], e[1]]), e[3]))
                .collect());
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 1..=count {
            let r = self.call(features::FEATURE_SET, 1, &[i])?;
            out.push((u16::from_be_bytes([r[4], r[5]]), r[7]));
        }
        Ok(out)
    }
}

impl DeviceInfo {
    fn hidraw_sysfs(&self) -> PathBuf {
        Path::new("/sys/class/hidraw").join(self.hidraw.file_name().unwrap())
    }
}

fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LHUB_DEBUG").is_some())
}

fn hex(b: &[u8]) -> String {
    let end = b.iter().rposition(|&x| x != 0).map_or(0, |i| i + 1).max(4);
    b[..end.min(b.len())]
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn wait_readable(file: &File, timeout: Duration) -> Result<bool> {
    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    let r = unsafe { libc::poll(&mut pfd, 1, ms) };
    if r < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(r > 0)
}

/// Logitech hidraw nodes that carry HID++ reports. Receivers themselves are skipped;
/// their paired devices appear as separate nodes via the kernel's logitech-dj driver.
pub fn enumerate() -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return out;
    };
    let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
    names.sort_by_key(|n| {
        n.to_string_lossy()
            .trim_start_matches("hidraw")
            .parse::<u32>()
            .unwrap_or(0)
    });
    for name in names {
        let sys = Path::new("/sys/class/hidraw").join(&name);
        let Ok(uevent) = fs::read_to_string(sys.join("device/uevent")) else {
            continue;
        };
        let field = |k: &str| {
            uevent
                .lines()
                .find_map(|l| l.strip_prefix(k))
                .unwrap_or("")
                .to_string()
        };
        let ids: Vec<u32> = field("HID_ID=")
            .split(':')
            .filter_map(|s| u32::from_str_radix(s, 16).ok())
            .collect();
        if ids.len() != 3 || ids[1] as u16 != LOGITECH {
            continue;
        }
        let desc = fs::read(sys.join("device/report_descriptor")).unwrap_or_default();
        if !has_report(&desc, REPORT_LONG) && !has_report(&desc, REPORT_CENTURION) {
            continue;
        }
        let real = fs::canonicalize(sys.join("device")).unwrap_or_default();
        // A receiver-attached device is a HID node nested under another 046d HID node.
        let parent_is_hid = real
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n.to_string_lossy().starts_with("0003:046D"));
        let product = ids[2] as u16;
        if is_receiver(product) && !parent_is_hid {
            continue;
        }
        let port = real
            .ancestors()
            .filter_map(|p| p.file_name()?.to_str())
            .find(|c| {
                c.contains('-')
                    && !c.contains(':')
                    && c.chars()
                        .all(|ch| ch.is_ascii_digit() || ch == '-' || ch == '.')
            })
            .unwrap_or("")
            .to_string();
        out.push(DeviceInfo {
            hidraw: Path::new("/dev").join(&name),
            vendor: LOGITECH,
            product,
            kernel_name: field("HID_NAME="),
            wireless: parent_is_hid,
            port,
        });
    }
    out
}

fn has_report(desc: &[u8], id: u8) -> bool {
    desc.windows(2).any(|w| w == [0x85, id])
}

fn is_receiver(product: u16) -> bool {
    matches!(
        product,
        0xc52b | 0xc534 | 0xc539 | 0xc53a | 0xc53f | 0xc541 | 0xc545 | 0xc547 | 0xc548
    )
}

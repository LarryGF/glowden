//! Finding devices and giving them stable ids across calls.

use crate::{Device, DeviceInfo, enumerate};

pub struct Found {
    pub device: Device,
    pub name: String,
    /// "hidrawN", or "hidrawN@slot" for a device behind a receiver without its own node.
    pub id: String,
}

fn try_open(info: &DeviceInfo, slot: Option<u8>) -> Option<Device> {
    let mut dev = match slot {
        None => Device::open(info.clone()).ok()?,
        Some(s) => Device::open_slot(info.clone(), s).ok()?,
    };
    // Wireless devices can be asleep; the first request wakes them but may time out.
    if dev.protocol().is_err() && (!info.wireless || dev.protocol().is_err()) {
        return None;
    }
    Some(dev)
}

fn node_name(info: &DeviceInfo) -> String {
    info.hidraw
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn found(mut device: Device, id: String) -> Found {
    let name = device
        .name()
        .ok()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| device.info.kernel_name.clone());
    Found { device, name, id }
}

/// Every device that answers HID++ 2.0. Nodes that don't answer directly may be
/// receivers whose paired devices have no node of their own.
pub fn open_all() -> Vec<Found> {
    let mut out = Vec::new();
    for info in enumerate() {
        if let Some(dev) = try_open(&info, None) {
            out.push(found(dev, node_name(&info)));
            continue;
        }
        if info.wireless {
            continue; // a per-device node already; receiver slots don't apply
        }
        for slot in 1..=6 {
            if let Some(dev) = try_open(&info, Some(slot)) {
                out.push(found(dev, format!("{}@{slot}", node_name(&info))));
            }
        }
    }
    out
}

pub fn open_id(id: &str) -> Option<Found> {
    let (node, slot) = match id.split_once('@') {
        Some((n, s)) => (n, Some(s.parse().ok()?)),
        None => (id, None),
    };
    let info = enumerate().into_iter().find(|i| node_name(i) == node)?;
    try_open(&info, slot).map(|dev| found(dev, id.to_string()))
}

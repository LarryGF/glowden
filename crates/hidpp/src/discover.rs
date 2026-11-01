//! Finding devices and giving them stable ids across calls.

use crate::{Device, DeviceInfo, enumerate};

pub struct Found {
    pub device: Device,
    pub name: String,
    /// "<usb port>:<product id>", plus "@slot" for a device behind a receiver without its own
    /// node. Built from the physical port so it survives hidraw renumbering.
    pub id: String,
}

fn try_open(info: &DeviceInfo, slot: Option<u8>) -> Option<Device> {
    let mut dev = match slot {
        None => Device::open(info.clone()).ok()?,
        Some(s) => Device::open_slot(info.clone(), s).ok()?,
    };
    // A sleeping wireless device wakes on the first request but may miss a few.
    let attempts = if info.wireless { 4 } else { 1 };
    (0..attempts).any(|_| dev.protocol().is_ok()).then_some(dev)
}

fn node_id(info: &DeviceInfo) -> String {
    format!("{}:{:04x}", info.port, info.product)
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
    scan_except(&|_| false)
}

/// Like `open_all`, but leaves alone nodes whose id `known` says are already handled.
pub fn scan_except(known: &dyn Fn(&str) -> bool) -> Vec<Found> {
    let mut out = Vec::new();
    for info in enumerate() {
        if known(&node_id(&info)) {
            continue;
        }
        if let Some(dev) = try_open(&info, None) {
            out.push(found(dev, node_id(&info)));
            continue;
        }
        if info.wireless {
            continue; // a per-device node already; receiver slots don't apply
        }
        for slot in 1..=6 {
            if known(&format!("{}@{slot}", node_id(&info))) {
                continue;
            }
            if let Some(dev) = try_open(&info, Some(slot)) {
                out.push(found(dev, format!("{}@{slot}", node_id(&info))));
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
    let info = enumerate().into_iter().find(|i| node_id(i) == node)?;
    try_open(&info, slot).map(|dev| found(dev, id.to_string()))
}

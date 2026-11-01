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
    // The transport already retries with growing timeouts while a wireless device wakes.
    dev.protocol().is_ok().then_some(dev)
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
    let _ = scan_each(known, &mut |f| out.push(f));
    out
}

/// Hands each device to `found_one` as soon as it answers, so callers can start using fast
/// devices while slow ones are still probed. Returns the ids of nodes that didn't answer
/// (e.g. a dongle whose headset is off), so callers can back off before probing them again.
pub fn scan_each(known: &dyn Fn(&str) -> bool, found_one: &mut dyn FnMut(Found)) -> Vec<String> {
    let mut silent = Vec::new();
    for info in enumerate() {
        let id = node_id(&info);
        if known(&id) {
            continue;
        }
        match try_open(&info, None) {
            Some(dev) => found_one(found(dev, id)),
            None => silent.push(id),
        }
    }
    silent
}

pub fn open_id(id: &str) -> Option<Found> {
    let (node, slot) = match id.split_once('@') {
        Some((n, s)) => (n, Some(s.parse().ok()?)),
        None => (id, None),
    };
    let info = enumerate().into_iter().find(|i| node_id(i) == node)?;
    try_open(&info, slot).map(|dev| found(dev, id.to_string()))
}

//! A virtual input device (Linux uinput) that performs assigned actions: keys, shortcuts,
//! mouse buttons and scrolling. One device serves every assignment.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex, OnceLock};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
pub const REL_HWHEEL: u16 = 0x06;
pub const REL_WHEEL: u16 = 0x08;
const REL_WHEEL_HI_RES: u16 = 0x0b;
const REL_HWHEEL_HI_RES: u16 = 0x0c;

// ioctl numbers from <linux/uinput.h>.
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_SET_RELBIT: libc::c_ulong = 0x4004_5566;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;

#[repr(C)]
struct Setup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

pub struct VirtualInput {
    file: File,
}

impl VirtualInput {
    fn create() -> std::io::Result<VirtualInput> {
        let file = OpenOptions::new().write(true).open("/dev/uinput")?;
        let fd = file.as_raw_fd();
        let ioctl = |req: libc::c_ulong, arg: libc::c_ulong| {
            if unsafe { libc::ioctl(fd, req, arg) } < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        ioctl(UI_SET_EVBIT, EV_KEY as _)?;
        ioctl(UI_SET_EVBIT, EV_REL as _)?;
        // Every keyboard key and mouse button glowden can name.
        for code in 1..0x2ff {
            ioctl(UI_SET_KEYBIT, code)?;
        }
        for rel in [REL_WHEEL, REL_HWHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES] {
            ioctl(UI_SET_RELBIT, rel as _)?;
        }
        let mut setup = Setup {
            bustype: 0x06, // BUS_VIRTUAL
            vendor: 0x046d,
            product: 0x0001,
            version: 1,
            name: [0; 80],
            ff_effects_max: 0,
        };
        let name = b"glowden virtual input";
        setup.name[..name.len()].copy_from_slice(name);
        if unsafe { libc::ioctl(fd, UI_DEV_SETUP, &setup as *const Setup) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        ioctl(UI_DEV_CREATE, 0)?;
        Ok(VirtualInput { file })
    }

    fn event(&mut self, kind: u16, code: u16, value: i32) -> std::io::Result<()> {
        let mut ev = [0u8; 24]; // struct input_event: timeval (zero = now), type, code, value
        ev[16..18].copy_from_slice(&kind.to_ne_bytes());
        ev[18..20].copy_from_slice(&code.to_ne_bytes());
        ev[20..24].copy_from_slice(&value.to_ne_bytes());
        self.file.write_all(&ev)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.event(EV_SYN, 0, 0)
    }

    pub fn key(&mut self, code: u16, down: bool) -> std::io::Result<()> {
        self.event(EV_KEY, code, down as i32)?;
        self.sync()
    }

    /// One wheel notch (`REL_WHEEL` or `REL_HWHEEL`), with its hi-res counterpart.
    pub fn scroll(&mut self, axis: u16, notches: i32) -> std::io::Result<()> {
        let hi = if axis == REL_WHEEL {
            REL_WHEEL_HI_RES
        } else {
            REL_HWHEEL_HI_RES
        };
        self.event(EV_REL, axis, notches)?;
        self.event(EV_REL, hi, notches * 120)?;
        self.sync()
    }
}

/// The shared device, created on first use. Errors (no access to /dev/uinput) are kept so
/// the UI can show them.
pub fn shared() -> Result<Arc<Mutex<VirtualInput>>, String> {
    static DEVICE: OnceLock<Result<Arc<Mutex<VirtualInput>>, String>> = OnceLock::new();
    DEVICE
        .get_or_init(|| {
            VirtualInput::create()
                .map(|d| Arc::new(Mutex::new(d)))
                .map_err(|e| format!("cannot create the virtual input device (/dev/uinput): {e}"))
        })
        .clone()
}

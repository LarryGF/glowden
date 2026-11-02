//! Settings of glowden itself: autostart, G HUB imports, notifications.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn unit_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    Some(base.join("systemd/user/glowdend.service"))
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Whether the service starts with the desktop session (systemd user unit enabled).
pub fn autostart() -> bool {
    systemctl(&["is-enabled", "--quiet", "glowdend.service"])
}

/// Enables or disables the user unit. Without a packaged unit (development builds), one is
/// written for this binary first.
pub fn set_autostart(on: bool) -> Result<(), String> {
    if on && !systemctl(&["cat", "glowdend.service"]) {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = unit_path().ok_or("cannot find the systemd user folder")?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let unit = format!(
            "[Unit]\nDescription=glowden device service\n\n[Service]\nExecStart={}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
            exe.display()
        );
        std::fs::write(&path, unit).map_err(|e| e.to_string())?;
        systemctl(&["daemon-reload"]);
    }
    let ok = if on {
        systemctl(&["enable", "glowdend.service"])
    } else {
        systemctl(&["disable", "glowdend.service"])
    };
    ok.then_some(())
        .ok_or_else(|| "systemctl --user failed".to_string())
}

/// Runs `glowdenctl import-ghub` on a folder and returns its report.
pub fn import_ghub(path: &str) -> Result<String, String> {
    if !std::path::Path::new(path).is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    let glowden = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("glowdenctl")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("glowdenctl"));
    let out = Command::new(glowden)
        .args(["import-ghub", path])
        .output()
        .map_err(|e| format!("cannot run glowden: {e}"))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

pub fn notify(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["--app-name=glowden", "--icon=battery-caution", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Caps Lock and Num Lock state from the kernel's keyboard LEDs (on if any keyboard shows it).
pub fn lock_keys() -> (bool, bool) {
    let mut caps = false;
    let mut num = false;
    for e in std::fs::read_dir("/sys/class/leds")
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = e.file_name().to_string_lossy().to_string();
        let on =
            || std::fs::read_to_string(e.path().join("brightness")).is_ok_and(|b| b.trim() != "0");
        if name.ends_with("::capslock") {
            caps |= on();
        } else if name.ends_with("::numlock") {
            num |= on();
        }
    }
    (caps, num)
}

pub fn notify_lock(what: &str, on: bool) {
    let _ = Command::new("notify-send")
        .args([
            "--app-name=glowden",
            "--icon=input-keyboard",
            "--expire-time=1500",
            "--transient",
        ])
        .arg(format!("{what} {}", if on { "on" } else { "off" }))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Whether the user's graphical session is locked (logind `LockedHint`, set by the desktop's
/// screen locker).
pub fn session_locked() -> bool {
    let uid = unsafe { libc::getuid() }.to_string();
    let Ok(out) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let id = f.next()?;
            (f.next()? == uid).then(|| id.to_string())
        })
        .any(|id| {
            Command::new("loginctl")
                .args([
                    "show-session",
                    &id,
                    "-p",
                    "Type",
                    "-p",
                    "LockedHint",
                    "--value",
                ])
                .output()
                .is_ok_and(|o| {
                    let text = String::from_utf8_lossy(&o.stdout);
                    let mut v = text.lines();
                    matches!(v.next(), Some("wayland" | "x11")) && v.next() == Some("yes")
                })
        })
}

//! Settings of lhub itself: autostart, G HUB imports, notifications.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn unit_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    Some(base.join("systemd/user/lhubd.service"))
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
    systemctl(&["is-enabled", "--quiet", "lhubd.service"])
}

/// Enables or disables the user unit. Without a packaged unit (development builds), one is
/// written for this binary first.
pub fn set_autostart(on: bool) -> Result<(), String> {
    if on && !systemctl(&["cat", "lhubd.service"]) {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = unit_path().ok_or("cannot find the systemd user folder")?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let unit = format!(
            "[Unit]\nDescription=lhub device service\n\n[Service]\nExecStart={}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
            exe.display()
        );
        std::fs::write(&path, unit).map_err(|e| e.to_string())?;
        systemctl(&["daemon-reload"]);
    }
    let ok = if on {
        systemctl(&["enable", "lhubd.service"])
    } else {
        systemctl(&["disable", "lhubd.service"])
    };
    ok.then_some(())
        .ok_or_else(|| "systemctl --user failed".to_string())
}

/// Runs `lhub import-ghub` on a folder and returns its report.
pub fn import_ghub(path: &str) -> Result<String, String> {
    if !std::path::Path::new(path).is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    let lhub = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("lhub")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("lhub"));
    let out = Command::new(lhub)
        .args(["import-ghub", path])
        .output()
        .map_err(|e| format!("cannot run lhub: {e}"))?;
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
        .args(["--app-name=lhub", "--icon=battery-caution", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

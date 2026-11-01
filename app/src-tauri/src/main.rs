//! UI shell: every device operation goes to the lhubd service, which owns the devices
//! and keeps settings applied after the window closes.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("lhub.sock")
}

/// The service binary next to ours (dev builds, packages), else whatever is on PATH.
fn daemon_binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| Some(p.parent()?.join("lhubd")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("lhubd"))
}

fn connect() -> Result<UnixStream, String> {
    if let Ok(s) = UnixStream::connect(socket_path()) {
        return Ok(s);
    }
    std::process::Command::new(daemon_binary())
        .spawn()
        .map_err(|e| format!("cannot start lhubd: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(s) = UnixStream::connect(socket_path()) {
            return Ok(s);
        }
    }
    Err("lhubd did not start".into())
}

fn send(req: &Value) -> Result<Value, String> {
    let mut stream = connect()?;
    stream.set_read_timeout(Some(Duration::from_secs(15))).ok();
    writeln!(stream, "{req}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let resp: Value =
        serde_json::from_str(&line).map_err(|e| format!("bad reply from lhubd: {e}"))?;
    if resp["ok"].as_bool() == Some(true) {
        Ok(resp["data"].clone())
    } else {
        Err(resp["error"]
            .as_str()
            .unwrap_or("unknown error")
            .to_string())
    }
}

#[tauri::command]
async fn request(req: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || send(&req))
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![request])
        .run(tauri::generate_context!())
        .expect("failed to start lhub");
}

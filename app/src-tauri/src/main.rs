//! UI shell: every device operation goes to the glowdend service, which owns the devices
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
        .join("glowden.sock")
}

/// The service binary next to ours (dev builds, packages), else whatever is on PATH.
fn daemon_binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| Some(p.parent()?.join("glowdend")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("glowdend"))
}

fn connect() -> Result<UnixStream, String> {
    if let Ok(s) = UnixStream::connect(socket_path()) {
        return Ok(s);
    }
    let mut child = std::process::Command::new(daemon_binary())
        .spawn()
        .map_err(|e| format!("cannot start glowdend: {e}"))?;
    // Reap the service if it exits while the app is open, so it doesn't linger as a zombie.
    std::thread::spawn(move || child.wait());
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(s) = UnixStream::connect(socket_path()) {
            return Ok(s);
        }
    }
    Err("glowdend did not start".into())
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
        serde_json::from_str(&line).map_err(|e| format!("bad reply from glowdend: {e}"))?;
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

/// Tray menu: devices with battery, profile switching, open, quit. Rebuilt periodically so
/// battery levels and the active profile stay current.
fn tray_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "open",
        "Open GlowDen",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let devices = send(&serde_json::json!({ "cmd": "list" })).unwrap_or_default();
    for d in devices.as_array().into_iter().flatten() {
        let name = d["name"].as_str().unwrap_or("device");
        let label = match d["battery"]["percent"].as_u64() {
            Some(p) if d["battery"]["charging"] == true => format!("{name}  {p}% (charging)"),
            Some(p) => format!("{name}  {p}%"),
            None => name.to_string(),
        };
        menu.append(&MenuItem::new(app, label, false, None::<&str>)?)?;
    }
    let profiles = send(&serde_json::json!({ "cmd": "profiles" })).unwrap_or_default();
    let active = profiles["active"].as_str();
    let sub = Submenu::new(app, "Profile", true)?;
    sub.append(&CheckMenuItem::with_id(
        app,
        "profile:",
        "Default",
        true,
        active.is_none(),
        None::<&str>,
    )?)?;
    for p in profiles["profiles"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap_or_default();
        let id = format!("profile:{name}");
        sub.append(&CheckMenuItem::with_id(
            app,
            id,
            name,
            true,
            active == Some(name),
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&sub)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;
    Ok(menu)
}

/// Set once the tray icon exists; without it, closing the window quits as usual.
static TRAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn show_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![request])
        .setup(|app| {
            let handle = app.handle().clone();
            let built = tauri::tray::TrayIconBuilder::with_id("glowden")
                // The emblem without the app icon's dark tile: trays draw on their own panel.
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("GlowDen")
                .menu(&tray_menu(&handle).map_err(|e| e.to_string())?)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_window(app),
                    "quit" => app.exit(0),
                    id if id.starts_with("profile:") => {
                        let name = &id["profile:".len()..];
                        let name = (!name.is_empty()).then(|| name.to_string());
                        let _ =
                            send(&serde_json::json!({ "cmd": "set_active_profile", "name": name }));
                    }
                    _ => {}
                })
                .build(app);
            let tray = match built {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("glowden: no tray icon ({e}); closing the window quits");
                    return Ok(());
                }
            };
            TRAY.store(true, std::sync::atomic::Ordering::Relaxed);
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(20));
                    if let Ok(menu) = tray_menu(&handle) {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
            });
            Ok(())
        })
        // Closing the window keeps GlowDen in the tray, as G HUB does; Quit is in the tray menu.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && TRAY.load(std::sync::atomic::Ordering::Relaxed)
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start glowden");
}

//! Which app has focus, for per-app profiles. On KDE (Wayland or X11) a small KWin script
//! reports every window activation to this service over the session bus; on GNOME the
//! packaged GlowDen focus extension does the same. Hyprland is followed through its event
//! socket.

use std::sync::mpsc::Sender;
use std::time::Duration;

use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;

const NAME: &str = "dev.glowden.Focus";
const GNOME_EXTENSION: &str = "glowden-focus@glowden.dev";

const SCRIPT: &str = r#"// Installed by glowden: reports the focused window's app to the glowden service.
function report(w) {
    if (!w) return;
    callDBus("dev.glowden.Focus", "/focus", "dev.glowden.Focus", "Activated",
             String(w.resourceClass || ""), String(w.caption || ""));
}
workspace.windowActivated.connect(report);
report(workspace.activeWindow);
"#;

/// The focused app's window class (e.g. "firefox") and title, sent on every change.
pub type Focus = (String, String);

pub fn spawn(tx: Sender<Focus>) {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        let tx = tx.clone();
        std::thread::Builder::new()
            .name("focus-hyprland".into())
            .spawn(move || {
                if let Err(e) = hyprland(tx) {
                    eprintln!("focus tracking (Hyprland) stopped: {e}");
                }
            })
            .expect("spawn focus thread");
    }
    std::thread::Builder::new()
        .name("focus".into())
        .spawn(move || {
            if let Err(e) = run(tx) {
                eprintln!("focus tracking unavailable: {e}");
            }
        })
        .expect("spawn focus thread");
}

fn run(tx: Sender<Focus>) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::new_session()?;
    conn.request_name(NAME, false, true, false)?;
    conn.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            if msg.member().as_deref() == Some("Activated")
                && let Ok((class, caption)) = msg.read2::<String, String>()
            {
                let _ = tx.send((class, caption));
            }
            let _ = conn.channel().send(msg.method_return());
            true
        }),
    );
    // Messages from any reporter are accepted, so the listener runs whichever one loads.
    match install_kwin_script(&conn) {
        Ok(()) => eprintln!("focus tracking: KWin script"),
        Err(kwin) => match enable_gnome_extension(&conn) {
            Ok(()) => eprintln!("focus tracking: GNOME extension {GNOME_EXTENSION}"),
            Err(gnome) => {
                eprintln!("focus tracking: no KWin ({kwin}) or GNOME extension ({gnome})")
            }
        },
    }
    loop {
        conn.process(Duration::from_secs(5))?;
    }
}

fn install_kwin_script(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
        .join(".local/share/glowden");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("kwin-focus.js");
    std::fs::write(&path, SCRIPT)?;
    let scripting = conn.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(5));
    // A previous service run may have left its copy loaded (also under the old lhub name).
    for name in ["glowden-focus", "lhub-focus"] {
        let _: Result<(bool,), _> =
            scripting.method_call("org.kde.kwin.Scripting", "unloadScript", (name,));
    }
    let (id,): (i32,) = scripting.method_call(
        "org.kde.kwin.Scripting",
        "loadScript",
        (path.to_string_lossy().to_string(), "glowden-focus"),
    )?;
    let script = conn.with_proxy(
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        Duration::from_secs(5),
    );
    script.method_call::<(), _, _, _>("org.kde.kwin.Script", "run", ())?;
    Ok(())
}

/// GNOME Shell only loads extensions it found at login, so a freshly installed package
/// works after logging out and in.
fn enable_gnome_extension(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let shell = conn.with_proxy(
        "org.gnome.Shell.Extensions",
        "/org/gnome/Shell/Extensions",
        Duration::from_secs(5),
    );
    let (ok,): (bool,) = shell.method_call(
        "org.gnome.Shell.Extensions",
        "EnableExtension",
        (GNOME_EXTENSION,),
    )?;
    if ok {
        Ok(())
    } else {
        Err("not loaded by GNOME Shell yet; log out and in after installing GlowDen".into())
    }
}

/// Hyprland's event socket sends `activewindow>>CLASS,TITLE` on every focus change.
fn hyprland(tx: Sender<Focus>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::BufRead;
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR")?;
    let path = std::path::Path::new(&runtime).join(format!("hypr/{sig}/.socket2.sock"));
    let socket = std::os::unix::net::UnixStream::connect(&path)?;
    eprintln!("focus tracking: Hyprland");
    for line in std::io::BufReader::new(socket).lines() {
        if let Some(rest) = line?.strip_prefix("activewindow>>") {
            let (class, title) = rest.split_once(',').unwrap_or((rest, ""));
            if tx.send((class.to_string(), title.to_string())).is_err() {
                break;
            }
        }
    }
    Ok(())
}

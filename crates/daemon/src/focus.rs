//! Which app has focus, for per-app profiles. On KDE (Wayland or X11) a small KWin script
//! reports every window activation to this service over the session bus.

use std::sync::mpsc::Sender;
use std::time::Duration;

use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;

const NAME: &str = "dev.lhub.Focus";

const SCRIPT: &str = r#"// Installed by lhub: reports the focused window's app to the lhub service.
function report(w) {
    if (!w) return;
    callDBus("dev.lhub.Focus", "/focus", "dev.lhub.Focus", "Activated",
             String(w.resourceClass || ""), String(w.caption || ""));
}
workspace.windowActivated.connect(report);
report(workspace.activeWindow);
"#;

/// The focused app's window class (e.g. "firefox") and title, sent on every change.
pub type Focus = (String, String);

pub fn spawn(tx: Sender<Focus>) {
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
    install_kwin_script(&conn)?;
    loop {
        conn.process(Duration::from_secs(5))?;
    }
}

fn install_kwin_script(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
        .join(".local/share/lhub");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("kwin-focus.js");
    std::fs::write(&path, SCRIPT)?;
    let scripting = conn.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(5));
    // A previous service run may have left its copy loaded.
    let _: Result<(bool,), _> =
        scripting.method_call("org.kde.kwin.Scripting", "unloadScript", ("lhub-focus",));
    let (id,): (i32,) = scripting.method_call(
        "org.kde.kwin.Scripting",
        "loadScript",
        (path.to_string_lossy().to_string(), "lhub-focus"),
    )?;
    let script = conn.with_proxy(
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        Duration::from_secs(5),
    );
    script.method_call::<(), _, _, _>("org.kde.kwin.Script", "run", ())?;
    Ok(())
}

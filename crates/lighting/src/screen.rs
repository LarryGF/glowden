//! The screen's colors on a grid, from `lhub-sampler` (the desktop's ScreenCast portal). One
//! capture is shared by every device showing the screen sampler effect.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};

use crate::engine::Rgb;

pub const COLS: usize = 16;
pub const ROWS: usize = 9;

pub struct ScreenSampler {
    child: Mutex<Child>,
    /// COLS × ROWS colors, row by row from the top left.
    pub grid: Arc<Mutex<Vec<Rgb>>>,
}

impl Drop for ScreenSampler {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}

static SHARED: Mutex<Weak<ScreenSampler>> = Mutex::new(Weak::new());

fn binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join("lhub-sampler")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("lhub-sampler"))
}

impl ScreenSampler {
    /// The running capture, or a new one. The first capture ever asks the desktop which
    /// screen to share; the portal's restore token skips that afterwards.
    pub fn shared() -> std::io::Result<Arc<ScreenSampler>> {
        let mut shared = SHARED.lock().unwrap();
        if let Some(s) = shared.upgrade() {
            return Ok(s);
        }
        let state =
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state/lhub");
        let _ = std::fs::create_dir_all(&state);
        let mut child = Command::new(binary())
            .args(["--grid", &format!("{COLS}x{ROWS}")])
            .arg("--token")
            .arg(state.join("screencast-token"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let grid = Arc::new(Mutex::new(vec![[0u8; 3]; COLS * ROWS]));
        let out = grid.clone();
        std::thread::spawn(move || {
            let mut frame = vec![0u8; COLS * ROWS * 3];
            while stdout.read_exact(&mut frame).is_ok() {
                let mut g = out.lock().unwrap();
                for (cell, px) in g.iter_mut().zip(frame.chunks_exact(3)) {
                    *cell = [px[0], px[1], px[2]];
                }
            }
        });
        let s = Arc::new(ScreenSampler {
            child: Mutex::new(child),
            grid,
        });
        *shared = Arc::downgrade(&s);
        Ok(s)
    }
}

//! Live frequency bands from whatever is playing, captured from the default output's
//! monitor through PipeWire's `pw-record`.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const RATE: f32 = 48000.0;
const N: usize = 1024;

pub struct AudioMeter {
    child: Child,
    /// 0..1 per band, low to high frequency, smoothed and auto-gained.
    pub bands: Arc<Mutex<Vec<f32>>>,
}

impl Drop for AudioMeter {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl AudioMeter {
    pub fn start(band_count: usize) -> std::io::Result<AudioMeter> {
        let mut child = Command::new("pw-record")
            .args([
                "-P",
                "{ stream.capture.sink=true node.name=glowden-visualizer }",
            ])
            .args(["--format", "f32", "--channels", "1", "--rate", "48000", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let bands = Arc::new(Mutex::new(vec![0.0; band_count]));
        let out = bands.clone();
        std::thread::spawn(move || {
            let edges = band_edges(band_count);
            let window: Vec<f32> = (0..N)
                .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos())
                .collect();
            let mut raw = vec![0u8; N * 4];
            let mut smooth = vec![0.0f32; band_count];
            let mut peak = 1e-3f32;
            while stdout.read_exact(&mut raw).is_ok() {
                let mut re: Vec<f32> = raw
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(&window)
                    .map(|(b, w)| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]) * w)
                    .collect();
                let mut im = vec![0.0f32; N];
                fft(&mut re, &mut im);
                for (b, &(lo, hi)) in edges.iter().enumerate() {
                    let mut e = 0.0f32;
                    for k in lo..hi {
                        e += (re[k] * re[k] + im[k] * im[k]).sqrt();
                    }
                    let v = (e / (hi - lo) as f32).ln_1p();
                    // Fast attack, slower release, so bars jump with beats and settle smoothly.
                    smooth[b] = if v > smooth[b] {
                        v
                    } else {
                        smooth[b] * 0.85 + v * 0.15
                    };
                }
                let frame_peak = smooth.iter().cloned().fold(0.0, f32::max);
                peak = (peak * 0.995).max(frame_peak).max(1e-3);
                let mut shared = out.lock().unwrap();
                for (dst, v) in shared.iter_mut().zip(&smooth) {
                    *dst = (v / peak).clamp(0.0, 1.0);
                }
            }
        });
        Ok(AudioMeter { child, bands })
    }
}

/// FFT bin ranges for `count` log-spaced bands between 40 Hz and 16 kHz.
fn band_edges(count: usize) -> Vec<(usize, usize)> {
    let bin = |f: f32| ((f / RATE * N as f32) as usize).clamp(1, N / 2);
    (0..count)
        .map(|i| {
            let f0 = 40.0 * (400.0f32).powf(i as f32 / count as f32);
            let f1 = 40.0 * (400.0f32).powf((i + 1) as f32 / count as f32);
            let (lo, hi) = (bin(f0), bin(f1));
            (lo, hi.max(lo + 1))
        })
        .collect()
}

/// In-place iterative radix-2 FFT; `re.len()` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

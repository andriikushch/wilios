//! Two pictures an agent can actually read: a waveform (is a section silent? is
//! the mix lopsided?) and a log-frequency spectrogram (is the patch bright or
//! dull? is energy folding down from Nyquist?).
//!
//! Both are pure-Rust: `rustfft` for the STFT, `png` for encoding. No plotting
//! library, no system libs.

use std::f32::consts::PI;

use rustfft::{FftPlanner, num_complex::Complex};

use crate::render::RenderSamples;

pub const WAVEFORM_W: u32 = 800;
pub const WAVEFORM_H: u32 = 240;
pub const SPECTROGRAM_W: u32 = 800;
pub const SPECTROGRAM_H: u32 = 360;

const N_FFT: usize = 4096;
const DB_RANGE: f32 = 80.0;
const F_MIN: f32 = 20.0;

/// Per-frame mono mix (average of channels).
fn mono(s: &RenderSamples) -> Vec<f32> {
    let ch = s.channels.max(1) as usize;
    if ch == 1 {
        return s.interleaved.clone();
    }
    s.interleaved
        .chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
        writer
            .write_image_data(rgb)
            .map_err(|e| format!("png data: {e}"))?;
    }
    Ok(out)
}

// --- waveform -------------------------------------------------------------

pub fn waveform_png(s: &RenderSamples, width: u32, height: u32) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("waveform dimensions must be non-zero".into());
    }
    let (w, h) = (width as usize, height as usize);
    let samples = mono(s);

    let bg = [17u8, 19, 24];
    let axis = [60u8, 64, 72];
    let wave = [120u8, 200, 255];
    let mut rgb = vec![0u8; w * h * 3];
    for px in rgb.chunks_mut(3) {
        px.copy_from_slice(&bg);
    }
    let put = |rgb: &mut [u8], x: usize, y: usize, c: [u8; 3]| {
        if x < w && y < h {
            let i = (y * w + x) * 3;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    };

    let mid = (h - 1) / 2;
    for x in 0..w {
        put(&mut rgb, x, mid, axis);
    }

    let y_of = |v: f32| -> usize {
        let v = v.clamp(-1.0, 1.0);
        (((1.0 - (v + 1.0) / 2.0) * (h - 1) as f32).round() as usize).min(h - 1)
    };

    let n = samples.len();
    for x in 0..w {
        let lo = n * x / w;
        let hi = (n * (x + 1) / w).max(lo + 1).min(n);
        if lo >= n {
            break;
        }
        let (mut mn, mut mx) = (f32::MAX, f32::MIN);
        for &v in &samples[lo..hi] {
            mn = mn.min(v);
            mx = mx.max(v);
        }
        if mn > mx {
            continue;
        }
        let (y0, y1) = (y_of(mx), y_of(mn));
        for y in y0..=y1 {
            put(&mut rgb, x, y, wave);
        }
    }

    encode_rgb(width, height, &rgb)
}

// --- spectrogram --------------------------------------------------------

/// dB-magnitude STFT resampled onto a log-frequency grid. `cells_db[col * height
/// + row]`, `row == 0` at the top (high frequency). Exposed for tests.
pub struct Spectrogram {
    pub width: usize,
    pub height: usize,
    pub sample_rate: u32,
    pub f_min: f32,
    pub f_max: f32,
    pub cells_db: Vec<f32>,
}

impl Spectrogram {
    /// Centre frequency of image row `row` (0 = top / high freq).
    pub fn row_hz(&self, row: usize) -> f32 {
        let frac = 1.0 - (row as f32 + 0.5) / self.height as f32;
        self.f_min * (self.f_max / self.f_min).powf(frac)
    }

    /// Row (0 = top) carrying the most energy in column `col`.
    pub fn column_peak_row(&self, col: usize) -> usize {
        let base = col * self.height;
        (0..self.height)
            .max_by(|&a, &b| {
                self.cells_db[base + a]
                    .partial_cmp(&self.cells_db[base + b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0)
    }
}

pub fn spectrogram(s: &RenderSamples, width: usize, height: usize) -> Spectrogram {
    let width = width.max(1);
    let height = height.max(1);
    let sr = s.sample_rate.max(1);
    let f_max = (sr as f32 / 2.0).max(F_MIN * 2.0);

    let samples = mono(s);
    let n_fft = N_FFT.min(samples.len().next_power_of_two().max(2));
    let half = n_fft / 2;

    // Hann window.
    let win: Vec<f32> = (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n_fft as f32).cos())
        .collect();

    let fft = FftPlanner::<f32>::new().plan_fft_forward(n_fft);

    let span = samples.len().saturating_sub(n_fft);
    let mut cells_db = vec![-DB_RANGE; width * height];
    let mut buf = vec![Complex::new(0.0f32, 0.0); n_fft];
    let mut peak_db = f32::MIN;

    for col in 0..width {
        let start = if width > 1 {
            span * col / (width - 1)
        } else {
            0
        };
        for (i, slot) in buf.iter_mut().enumerate() {
            let x = samples.get(start + i).copied().unwrap_or(0.0);
            *slot = Complex::new(x * win[i], 0.0);
        }
        fft.process(&mut buf);

        // Magnitude per FFT bin, normalised so a full-scale sinusoid ≈ 0 dB.
        let norm = 2.0 / n_fft as f32;
        for row in 0..height {
            let frac = 1.0 - (row as f32 + 0.5) / height as f32;
            let f = F_MIN * (f_max / F_MIN).powf(frac);
            let bin = ((f * n_fft as f32 / sr as f32).round() as usize).min(half.saturating_sub(1));
            let mag = buf[bin].norm() * norm;
            let db = 20.0 * (mag + 1e-9).log10();
            cells_db[col * height + row] = db;
            if db > peak_db {
                peak_db = db;
            }
        }
    }

    // Clamp to a DB_RANGE window below the loudest cell.
    if peak_db > f32::MIN {
        let floor = peak_db - DB_RANGE;
        for c in &mut cells_db {
            *c = c.max(floor);
        }
    }

    Spectrogram {
        width,
        height,
        sample_rate: sr,
        f_min: F_MIN,
        f_max,
        cells_db,
    }
}

fn magma(t: f32) -> [u8; 3] {
    // Compact 5-stop approximation of the magma colormap.
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.00, [0.00, 0.00, 0.02]),
        (0.25, [0.28, 0.06, 0.36]),
        (0.50, [0.68, 0.15, 0.38]),
        (0.75, [0.97, 0.44, 0.19]),
        (1.00, [0.99, 0.95, 0.75]),
    ];
    let t = t.clamp(0.0, 1.0);
    let mut i = 0;
    while i + 1 < STOPS.len() && t > STOPS[i + 1].0 {
        i += 1;
    }
    let (t0, c0) = STOPS[i];
    let (t1, c1) = STOPS[(i + 1).min(STOPS.len() - 1)];
    let k = if (t1 - t0).abs() < f32::EPSILON {
        0.0
    } else {
        (t - t0) / (t1 - t0)
    };
    let mut out = [0u8; 3];
    for ch in 0..3 {
        out[ch] = (((c0[ch] + (c1[ch] - c0[ch]) * k) * 255.0).round() as i32).clamp(0, 255) as u8;
    }
    out
}

pub fn spectrogram_png(s: &RenderSamples, width: u32, height: u32) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("spectrogram dimensions must be non-zero".into());
    }
    let spec = spectrogram(s, width as usize, height as usize);
    let peak = spec
        .cells_db
        .iter()
        .copied()
        .fold(f32::MIN, f32::max)
        .max(-DB_RANGE + 1.0);
    let floor = peak - DB_RANGE;

    let mut rgb = vec![0u8; spec.width * spec.height * 3];
    for row in 0..spec.height {
        for col in 0..spec.width {
            let db = spec.cells_db[col * spec.height + row];
            let t = ((db - floor) / (peak - floor)).clamp(0.0, 1.0);
            let c = magma(t);
            let i = (row * spec.width + col) * 3;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    }
    encode_rgb(width, height, &rgb)
}

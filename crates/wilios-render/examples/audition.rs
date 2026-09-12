//! Offline audition helper for iterating on synth presets without an audio
//! device or the `render` MCP tool.
//!
//! ```text
//! cargo run -p wilios-render --example audition -- path/to/piece.wilios [seconds] [out_dir]
//! ```
//!
//! Renders the piece, writes a WAV and a log-frequency spectrogram PNG next to
//! it (or into `out_dir`), and prints scalar metrics: peak / RMS dBFS, DC
//! offset, a crude high-frequency-energy ratio (a proxy for aliasing / "fizz"),
//! and how hard the mixer's limiter engaged.

use std::path::{Path, PathBuf};

use wilios_render::image::spectrogram_png;
use wilios_render::pipeline::load_interpreter;
use wilios_render::render::{RenderOpts, render_to_samples};

fn main() {
    let mut args = std::env::args().skip(1);
    let src = args.next().unwrap_or_else(|| {
        eprintln!("usage: audition <file.wilios> [seconds] [out_dir]");
        std::process::exit(2);
    });
    let secs: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6.0);
    let out_dir: PathBuf = args.next().map(PathBuf::from).unwrap_or_else(|| {
        Path::new(&src)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    });

    let src_path = Path::new(&src);
    let stem = src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audition");

    let interp = load_interpreter(src_path).unwrap_or_else(|e| {
        eprintln!("load error: {e}");
        std::process::exit(1);
    });

    let mut opts = RenderOpts::new(out_dir.join(format!("{stem}.wav")));
    opts.duration = Some(secs);
    let samples = render_to_samples(interp, &opts).unwrap_or_else(|e| {
        eprintln!("render error: {e}");
        std::process::exit(1);
    });

    // WAV
    let wav = wilios_render::render::encode_wav_bytes(&samples).unwrap();
    std::fs::write(&opts.out, wav).unwrap();

    // Spectrogram PNG
    let png = spectrogram_png(&samples, 900, 360).unwrap();
    let png_path = out_dir.join(format!("{stem}.spectrogram.png"));
    std::fs::write(&png_path, png).unwrap();

    // Scalar metrics over the mono signal (both channels are identical).
    let ch = samples.channels.max(1) as usize;
    let mono: Vec<f32> = samples.interleaved.chunks(ch).map(|f| f[0]).collect();
    let n = mono.len().max(1) as f64;
    let mut peak = 0f32;
    let mut sum = 0f64;
    let mut sum_sq = 0f64;
    let mut diff_sq = 0f64;
    let mut limiter_hits = 0u64;
    let mut prev = 0f32;
    for (i, &x) in mono.iter().enumerate() {
        peak = peak.max(x.abs());
        sum += x as f64;
        sum_sq += (x as f64).powi(2);
        if i > 0 {
            diff_sq += ((x - prev) as f64).powi(2);
        }
        if x.abs() > 0.8 {
            limiter_hits += 1;
        }
        prev = x;
    }
    let rms = (sum_sq / n).sqrt();
    let dc = sum / n;
    // sum((x[n]-x[n-1])^2) / sum(x[n]^2): rises with high-frequency content;
    // an aliased/"fizzy" render pushes this up.
    let hf_ratio = if sum_sq > 0.0 { diff_sq / sum_sq } else { 0.0 };
    let dbfs = |a: f64| if a <= 0.0 { -160.0 } else { 20.0 * a.log10() };

    println!(
        "{stem}: {:.2}s  {} frames @ {} Hz",
        secs,
        samples.frames(),
        samples.sample_rate
    );
    println!("  peak      {:>7.2} dBFS", dbfs(peak as f64));
    println!("  rms       {:>7.2} dBFS", dbfs(rms));
    println!("  dc offset {:>7.5}", dc);
    println!(
        "  hf ratio  {:>7.4}   (lower = darker / less alias fizz)",
        hf_ratio
    );
    println!(
        "  limiter   {:>7.4}   (fraction of samples past 0.8)",
        limiter_hits as f64 / n
    );
    println!("  wrote {}", opts.out.display());
    println!("  wrote {}", png_path.display());
}

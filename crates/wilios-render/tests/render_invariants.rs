//! Invariant checks for the offline render path — no golden files.

use std::path::PathBuf;

use wilios_render::analysis::analyze;
use wilios_render::image::{self, spectrogram};
use wilios_render::pipeline::load_interpreter;
use wilios_render::render::{
    RenderOpts, RenderProgress, RenderSamples, render_to_samples, render_to_samples_with_progress,
};

fn src_to_samples(name: &str, source: &str, opts: RenderOpts) -> RenderSamples {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, source).unwrap();
    let interp = load_interpreter(&path).expect("load");
    render_to_samples(interp, &opts).expect("render")
}

fn src_to_progress(name: &str, source: &str, opts: RenderOpts) -> Vec<RenderProgress> {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, source).unwrap();
    let interp = load_interpreter(&path).expect("load");
    let mut pings = Vec::new();
    render_to_samples_with_progress(interp, &opts, &mut |p| pings.push(p)).expect("render");
    pings
}

fn opts(sample_rate: u32, duration: Option<f32>) -> RenderOpts {
    RenderOpts {
        out: PathBuf::from("unused.wav"),
        sample_rate,
        duration,
        max_render_secs: 30.0,
    }
}

#[test]
fn sine_440_peaks_in_the_440hz_spectrogram_bin() {
    // A4 is concert pitch, 440 Hz.
    let s = src_to_samples(
        "inv_a4.wilios",
        "tempo 120\ntrack 1\nwave sine\n<A4> 1/1\n",
        opts(44_100, Some(1.5)),
    );
    let spec = spectrogram(&s, 400, 360);
    // A column from the sustained middle of the note.
    let row = spec.column_peak_row(spec.width / 2);
    let hz = spec.row_hz(row);
    assert!(
        (430.0..=450.0).contains(&hz),
        "dominant frequency {hz:.1} Hz not in 430–450 Hz (row {row})"
    );
}

#[test]
fn full_rest_is_silent() {
    let s = src_to_samples("inv_rest.wilios", "track 1\nrest 1/1\n", opts(44_100, None));
    let a = analyze(&s, &[], true);
    assert!(
        a.peak_dbfs <= -120.0,
        "expected near-silence, got peak {} dBFS",
        a.peak_dbfs
    );
}

#[test]
fn hot_stacked_mix_reports_clipped_samples() {
    // Eight loud unison voices — well past what the limiter/saturator can hold
    // linearly.
    let mut src = String::from("tempo 120\n");
    for t in 1..=8 {
        src.push_str(&format!("track {t}\nvolume 127\nwave sine\n<C3> 1/2\n"));
    }
    let s = src_to_samples("inv_hot.wilios", &src, opts(44_100, None));
    let a = analyze(&s, &[], true);
    assert!(
        a.clipped_samples > 0,
        "a deliberately hot mix should drive the saturator (peak {} dBFS)",
        a.peak_dbfs
    );
}

#[test]
fn pngs_decode_to_requested_dimensions() {
    let s = src_to_samples(
        "inv_png.wilios",
        "tempo 140\ntrack 1\n<C4> 1/4\n<E4> 1/4\n<G4> 1/4\n<C5> 1/4\n",
        opts(44_100, None),
    );

    for (w, h, bytes) in [
        (320u32, 120u32, image::waveform_png(&s, 320, 120).unwrap()),
        (256, 200, image::spectrogram_png(&s, 256, 200).unwrap()),
    ] {
        let decoder = png::Decoder::new(bytes.as_slice());
        let reader = decoder.read_info().expect("valid png");
        let info = reader.info();
        assert_eq!((info.width, info.height), (w, h));
    }
}

#[test]
fn progress_callback_is_monotonic_and_completes() {
    // 1.5 s at 44.1 kHz = 66_150 frames, an exact total (fixed --duration).
    let pings = src_to_progress(
        "inv_progress.wilios",
        "tempo 120\ntrack 1\nwave sine\n<A4> 1/1\n",
        opts(44_100, Some(1.5)),
    );

    assert!(
        pings.len() >= 2,
        "expected several progress pings, got {}",
        pings.len()
    );

    let mut prev = 0u64;
    for (i, p) in pings.iter().enumerate() {
        assert_eq!(
            p.frames_total,
            Some(66_150),
            "fixed duration => exact total"
        );
        assert!(
            p.frames_done >= prev,
            "frames_done went backwards: {} -> {}",
            prev,
            p.frames_done
        );
        prev = p.frames_done;
        // Only the trailing ping is flagged done.
        assert_eq!(p.done, i == pings.len() - 1, "done flag on ping {i}");
    }

    let last = pings.last().unwrap();
    assert_eq!(
        last.frames_done, 66_150,
        "final ping must land on the exact total"
    );
    assert_eq!(last.fraction(), Some(1.0));
}

#[test]
fn open_ended_progress_has_no_total() {
    // No --duration: the render is bounded only by max_render_secs (30 s here),
    // so there is no meaningful percentage.
    let pings = src_to_progress(
        "inv_progress_endless.wilios",
        "track 1\nloop (true) {\n  <C4> 1/8\n}\n",
        opts(8_000, None),
    );

    assert!(!pings.is_empty());
    for p in &pings {
        assert_eq!(p.frames_total, None);
        assert_eq!(p.fraction(), None, "open-ended render has no fraction");
    }
    assert!(
        pings.last().unwrap().done,
        "trailing ping must be flagged done"
    );
    // Truncated near max_render_secs (30 s) * 8_000 Hz = 240_000 frames.
    let frames = pings.last().unwrap().frames_done;
    assert!(
        (224_000..=256_000).contains(&frames),
        "expected truncation near 240_000 frames, got {frames}"
    );
}

#[test]
fn endless_loop_truncates_without_error() {
    let s = src_to_samples(
        "inv_endless.wilios",
        "track 1\nloop (true) {\n  <C4> 1/8\n}\n",
        opts(8_000, None),
    );
    assert!(!s.finished, "an endless loop must report finished == false");
    let secs = s.frames() as f64 / s.sample_rate as f64;
    assert!(
        (28.0..=32.0).contains(&secs),
        "expected truncation near max_render_secs (30s), got {secs:.1}s"
    );
}

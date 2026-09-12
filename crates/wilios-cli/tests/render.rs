//! End-to-end checks for the device-independent offline renderer.

use std::path::PathBuf;

use wilios_cli::pipeline::load_interpreter;
use wilios_cli::render::{RenderOpts, render};

fn write_program(name: &str, src: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, src).unwrap();
    path
}

#[test]
fn renders_finite_program_to_stereo_16bit_wav() {
    let src = write_program("finite.wilios", "track 1\n<C4> 1/4\n<E4> 1/4\n<G4> 1/4\n");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("finite.wav");
    let _ = std::fs::remove_file(&out);

    let interp = load_interpreter(&src).expect("load");
    render(
        interp,
        RenderOpts {
            out: out.clone(),
            sample_rate: 44_100,
            duration: Some(1.0),
            max_render_secs: 5.0,
        },
    )
    .expect("render");

    let reader = hound::WavReader::open(&out).expect("open wav");
    let spec = reader.spec();
    assert_eq!(spec.channels, 2);
    assert_eq!(spec.sample_rate, 44_100);
    assert_eq!(spec.bits_per_sample, 16);

    let samples: Vec<i16> = reader.into_samples::<i16>().map(Result::unwrap).collect();
    // 1.0s * 44100 frames * 2 channels
    assert_eq!(samples.len(), 44_100 * 2);
    assert!(samples.iter().any(|&s| s != 0), "output is all silence");

    // No stray temp file left behind.
    assert!(!out.with_extension("wav.partial").exists());
}

#[test]
fn endless_loop_without_duration_errors_and_writes_no_file() {
    let src = write_program("endless.wilios", "track 1\nloop (true) {\n  <C4> 1/4\n}\n");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("endless.wav");
    let _ = std::fs::remove_file(&out);

    let interp = load_interpreter(&src).expect("load");
    let err = render(
        interp,
        RenderOpts {
            out: out.clone(),
            sample_rate: 8_000,
            duration: None,
            max_render_secs: 1.0,
        },
    )
    .expect_err("endless loop must be rejected without --duration");

    assert!(err.contains("--duration"), "unexpected error: {err}");
    assert!(!out.exists(), "no WAV should be produced");
    assert!(!out.with_extension("wav.partial").exists());
}

#[test]
fn endless_loop_with_duration_renders_exact_length() {
    let src = write_program("endless2.wilios", "track 1\nloop (true) {\n  <C4> 1/8\n}\n");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("endless2.wav");
    let _ = std::fs::remove_file(&out);

    let interp = load_interpreter(&src).expect("load");
    render(
        interp,
        RenderOpts {
            out: out.clone(),
            sample_rate: 8_000,
            duration: Some(0.5),
            max_render_secs: 5.0,
        },
    )
    .expect("render");

    let reader = hound::WavReader::open(&out).expect("open wav");
    assert_eq!(reader.duration(), 4_000); // 0.5s * 8000 frames
}

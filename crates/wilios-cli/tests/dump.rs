//! End-to-end checks for the device-independent event dump.

use std::path::PathBuf;

use wilios_cli::dump::{DumpFormat, DumpOpts, dump};
use wilios_cli::pipeline::load_interpreter;

fn write_program(name: &str, src: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, src).unwrap();
    path
}

fn opts(format: DumpFormat, duration: Option<f32>) -> DumpOpts {
    DumpOpts {
        format,
        duration,
        max_secs: 5.0,
    }
}

#[test]
fn dumps_finite_two_track_program_as_json() {
    let src = write_program(
        "dump_finite.wilios",
        "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\ntrack 2\n<G3> 1/2\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let json = dump(interp, opts(DumpFormat::Json, None)).expect("dump");

    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(v["finished"], serde_json::json!(true));
    assert_eq!(v["note_count"], serde_json::json!(3));

    let tracks = v["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["id"], serde_json::json!(1));

    let t1 = tracks[0]["notes"].as_array().unwrap();
    assert_eq!(t1.len(), 2);
    assert_eq!(t1[0]["at_ms"], serde_json::json!(0));
    assert_eq!(t1[0]["pitch"], serde_json::json!("C4"));
    assert_eq!(t1[0]["at_beats"], serde_json::json!("0"));
    // 120 BPM: a 1/4 note is 500 ms.
    assert_eq!(t1[0]["dur_ms"], serde_json::json!(500));
    assert_eq!(t1[0]["dur_beats"], serde_json::json!("1/4"));
    assert_eq!(t1[1]["at_ms"], serde_json::json!(500));
    assert_eq!(t1[1]["pitch"], serde_json::json!("E4"));

    assert_eq!(tracks[1]["id"], serde_json::json!(2));
    // Full-fidelity fields are present.
    assert!(t1[0]["release_ms"].is_number());
    assert!(t1[0]["time_signature"].is_string());
}

#[test]
fn text_format_lists_tracks_and_pitches() {
    let src = write_program("dump_text.wilios", "track 1\n<C4> 1/4\n<F#4> 1/4\n");
    let interp = load_interpreter(&src).expect("load");
    let text = dump(interp, opts(DumpFormat::Text, None)).expect("dump");

    assert!(text.contains("track 1"), "missing track header:\n{text}");
    assert!(text.contains("C4"), "missing C4:\n{text}");
    assert!(text.contains("F#4"), "missing F#4:\n{text}");
}

#[test]
fn endless_loop_without_duration_errors() {
    let src = write_program(
        "dump_endless.wilios",
        "track 1\nloop (true) {\n  <C4> 1/4\n}\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let err = dump(interp, opts(DumpFormat::Json, None)).expect_err("must error");
    assert!(err.contains("--duration"), "unexpected error: {err}");
}

#[test]
fn endless_loop_with_duration_dumps_bounded_events() {
    let src = write_program(
        "dump_endless2.wilios",
        "track 1\nloop (true) {\n  <C4> 1/8\n}\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let json = dump(interp, opts(DumpFormat::Json, Some(2.0))).expect("dump");

    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["finished"], serde_json::json!(false));
    let notes = v["tracks"][0]["notes"].as_array().unwrap();
    assert!(!notes.is_empty(), "expected some notes");
    assert!(
        notes.iter().all(|n| n["at_ms"].as_u64().unwrap() < 2_000),
        "all onsets within the 2s bound"
    );
}

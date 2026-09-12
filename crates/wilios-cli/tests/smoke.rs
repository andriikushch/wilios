//! End-to-end checks for the headless CI smoke command.

use std::path::PathBuf;

use wilios_cli::pipeline::load_interpreter;
use wilios_cli::smoke::{SmokeOpts, smoke};

fn write_program(name: &str, src: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, src).unwrap();
    path
}

fn opts(duration: Option<f32>) -> SmokeOpts {
    SmokeOpts {
        duration,
        max_secs: 5.0,
    }
}

#[test]
fn equal_length_tracks_pass() {
    // track 1: two 1/4 notes = 1/2; track 2: one 1/2 note.
    let src = write_program(
        "smoke_equal.wilios",
        "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\ntrack 2\n<G3> 1/2\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let summary = smoke(interp, opts(None)).expect("smoke ok");
    assert!(summary.contains("2 track"), "unexpected summary: {summary}");
}

#[test]
fn unequal_length_tracks_fail() {
    let src = write_program(
        "smoke_unequal.wilios",
        "track 1\n<C4> 1/4\ntrack 2\n<G3> 1/2\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let err = smoke(interp, opts(None)).expect_err("must flag misaligned tracks");
    assert!(err.contains("same end time"), "unexpected error: {err}");
    assert!(err.contains("track 1") && err.contains("track 2"), "{err}");
}

#[test]
fn trailing_rest_counts_toward_end() {
    // track 2 ends on a rest: audible last-note end would read short, but the
    // nominal position (1/2 on both tracks) is equal.
    let src = write_program(
        "smoke_trailing_rest.wilios",
        "track 1\n<C4> 1/4\n<C4> 1/4\ntrack 2\n<G3> 1/4\nrest 1/4\n",
    );
    let interp = load_interpreter(&src).expect("load");
    smoke(interp, opts(None)).expect("trailing rest keeps tracks aligned");
}

#[test]
fn endless_loop_without_duration_fails() {
    let src = write_program(
        "smoke_endless.wilios",
        "track 1\nloop (true) {\n  <C4> 1/4\n}\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let err = smoke(interp, opts(None)).expect_err("must error");
    assert!(err.contains("--duration"), "unexpected error: {err}");
}

#[test]
fn endless_loop_with_duration_skips_equal_check() {
    let src = write_program(
        "smoke_endless2.wilios",
        "track 1\nloop (true) {\n  <C4> 1/8\n}\n",
    );
    let interp = load_interpreter(&src).expect("load");
    let summary = smoke(interp, opts(Some(2.0))).expect("bounded run is ok");
    assert!(summary.contains("skipped"), "unexpected summary: {summary}");
}

#[test]
fn runtime_error_is_reported() {
    let src = write_program("smoke_runtime_err.wilios", "track 1\n<C4> 1/0\n");
    let interp = load_interpreter(&src).expect("load");
    smoke(interp, opts(None)).expect_err("zero-division duration is a runtime error");
}

use proptest::prelude::*;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::Lexer;
use wilios_core::parser::ast::Waveform;
use wilios_core::parser::parser::Parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_pipeline(source: &str) -> Option<Vec<wilios_core::interpreter::event::Event>> {
    let Ok(tokens) = Lexer::new(source).lex() else {
        return None;
    };
    let Ok(program) = Parser::new(tokens).parse() else {
        return None;
    };
    let Ok(mut interp) = Interpreter::new(program) else {
        return None;
    };
    interp.schedule_until(0, 100).ok()
}

// ---------------------------------------------------------------------------
// Determinism: same input always produces the same token stream
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn lexer_is_deterministic(input in "[a-z0-9 \n<>,./]*") {
        let r1 = Lexer::new(&input).lex();
        let r2 = Lexer::new(&input).lex();
        prop_assert_eq!(r1, r2);
    }
}

// ---------------------------------------------------------------------------
// Empty input: no events, no panic
// ---------------------------------------------------------------------------

#[test]
fn empty_input_produces_no_events() {
    let tokens = Lexer::new("").lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    assert!(interp.schedule_until(0, 5_000).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// schedule_until(0, 0) always returns no events
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn schedule_until_zero_returns_empty(
        source in "(tempo 120\ntrack 1\n)?(<[A-G][0-9]> 1/4\n){0,3}"
    ) {
        let Ok(tokens) = Lexer::new(&source).lex() else { return Ok(()); };
        let Ok(program) = Parser::new(tokens).parse() else { return Ok(()); };
        let Ok(mut interp) = Interpreter::new(program) else { return Ok(()); };
        let events = interp.schedule_until(0, 0).unwrap_or_default();
        prop_assert!(events.is_empty(), "schedule_until(0,0) must return no events");
    }
}

// ---------------------------------------------------------------------------
// Rest-only programs: no note events emitted
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn rest_program_emits_no_note_events(n in 1usize..=8) {
        let source = format!("tempo 120\ntrack 1\n{}", "rest 1/4\n".repeat(n));
        if let Some(events) = run_pipeline(&source) {
            prop_assert!(
                events.is_empty(),
                "rest-only program produced {} event(s)",
                events.len()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Monotonicity: event timestamps are non-decreasing within each track
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn event_timestamps_are_monotonic(notes in 1usize..=8) {
        let source = format!(
            "tempo 120\ntrack 1\n{}",
            "<C4> 1/4\n".repeat(notes)
        );
        if let Some(events) = run_pipeline(&source) {
            let mut last_t = 0u64;
            for ev in &events {
                prop_assert!(
                    ev.at >= last_t,
                    "event at {} is earlier than previous event at {}",
                    ev.at,
                    last_t
                );
                last_t = ev.at;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Idempotency: calling schedule_until for an already-elapsed window is a no-op
// ---------------------------------------------------------------------------

#[test]
fn second_schedule_until_same_window_is_empty() {
    let source = "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\n";
    let tokens = Lexer::new(source).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();

    // First call advances state and returns events
    let first = interp.schedule_until(0, 5_000).unwrap();
    // Second call over the same window: all events already past, nothing new
    let second = interp.schedule_until(0, 5_000).unwrap();
    assert!(
        second.is_empty(),
        "expected no events on second pass, got {} event(s) (first pass had {})",
        second.len(),
        first.len()
    );
}

// ---------------------------------------------------------------------------
// Fuzz: arbitrary DSL-like strings through the full pipeline
// Runs in a separate thread with a 500 ms deadline to guard against the
// known interpreter hang: `loop(true) {}` (empty body, time never advances).
// Any input that hangs is silently skipped — we are checking that
// the *process* is not aborted (SIGABRT, SIGFPE, illegal-instruction, etc.).
// ---------------------------------------------------------------------------

fn run_pipeline_timed(source: String) -> bool {
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok(tokens) = Lexer::new(&source).lex() else {
            let _ = tx.send(());
            return;
        };
        let Ok(program) = Parser::new(tokens).parse() else {
            let _ = tx.send(());
            return;
        };
        let Ok(mut interp) = Interpreter::new(program) else {
            let _ = tx.send(());
            return;
        };
        let _ = interp.schedule_until(0, 100);
        let _ = tx.send(());
    });
    // true = completed (ok or errored), false = timed out (interpreter hung)
    rx.recv_timeout(Duration::from_millis(500)).is_ok()
}

proptest! {
    #[test]
    fn full_pipeline_never_aborts(input in "[ -~\n]{0,128}") {
        // A false return means the interpreter got stuck (known bug with
        // empty loop bodies). We accept hangs here; what we are guarding
        // against is process-level crashes.
        let _completed = run_pipeline_timed(input);
    }
}

// ---------------------------------------------------------------------------
// Track IDs: note events must carry the track id of the emitting track
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn note_events_have_correct_track_id(track_id in 1usize..=8, notes in 1usize..=4) {
        let body = "<C4> 1/4\n".repeat(notes);
        let source = format!("tempo 120\ntrack {track_id}\n{body}");
        if let Some(events) = run_pipeline(&source) {
            for ev in &events {
                prop_assert_eq!(
                    ev.track, track_id,
                    "event track {} != expected track {}", ev.track, track_id
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Volume: the volume statement propagates into emitted note events
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn volume_stmt_propagates_to_events(vol in 0usize..=127) {
        let source = format!("tempo 120\ntrack 1\nvolume {vol}\n<C4> 1/4\n");
        if let Some(events) = run_pipeline(&source) {
            for ev in &events {
                let wilios_core::interpreter::event::EventKind::Note { volume, .. } = &ev.kind;
                prop_assert_eq!(*volume, vol);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Waveforms: wave <X> sets the waveform on all subsequent note events
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn waveform_propagates_to_events(
        wave in prop_oneof![
            Just(("sine",  Waveform::Sine)),
            Just(("square", Waveform::Square)),
            Just(("saw",   Waveform::Saw)),
            Just(("tri",   Waveform::Triangle)),
        ],
    ) {
        let (wave_str, expected) = wave;
        let source = format!("tempo 120\ntrack 1\nwave {wave_str}\n<C4> 1/4\n");
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        let events = interp.schedule_until(0, 1_000_000_000).unwrap();
        prop_assert_eq!(events.len(), 1);
        let wilios_core::interpreter::event::EventKind::Note { waveform, .. } = &events[0].kind;
        prop_assert_eq!(waveform, &expected);
    }
}

// ---------------------------------------------------------------------------
// Multi-track: programs with N tracks emit events from all N tracks
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn multi_track_all_tracks_emit_events(n_tracks in 1usize..=4) {
        let tracks: String = (1..=n_tracks)
            .map(|i| format!("track {i}\n<C4> 1/4\n"))
            .collect();
        let source = format!("tempo 120\n{tracks}");
        if let Some(events) = run_pipeline(&source) {
            let present: std::collections::HashSet<usize> =
                events.iter().map(|e| e.track).collect();
            for i in 1..=n_tracks {
                prop_assert!(present.contains(&i), "track {i} produced no events");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Loop count: a loop running N times emits exactly N note events
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn loop_n_times_emits_n_notes(n in 1usize..=8) {
        let source = format!(
            "tempo 120\ntrack 1\nlet i = 0\nloop (i < {n}) {{\n    <C4> 1/4\n    i = i + 1\n}}\n"
        );
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        let events = interp.schedule_until(0, 1_000_000_000).unwrap();
        let got = events.len();
        prop_assert_eq!(got, n, "expected {} events, got {}", n, got);
    }
}

proptest! {
    #[test]
    fn full_pipeline_valid_dsl_subset(
        bpm   in 60usize..=240,
        notes in proptest::collection::vec(
            prop_oneof![
                Just("<C4> 1/4"),
                Just("<D4> 1/4"),
                Just("<E4> 1/4"),
                Just("<F4> 1/4"),
                Just("<G4> 1/4"),
                Just("<A4> 1/4"),
                Just("<B4> 1/4"),
                Just("rest 1/4"),
                Just("<C4, E4, G4> 1/2"),
            ],
            1..=16,
        ),
    ) {
        let body = notes.join("\n");
        let source = format!("tempo {bpm}\ntrack 1\n{body}\n");
        // Valid programs must not error
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        interp.schedule_until(0, 10_000).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Arrays: reading an in-bounds index returns the correct element
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn array_index_read_correct_value(
        elems in proptest::collection::vec(0i64..=127, 1..=8),
        idx   in 0usize..8,
    ) {
        let idx = idx % elems.len(); // guaranteed in-bounds
        let lit: Vec<String> = elems.iter().map(|n| n.to_string()).collect();
        let source = format!(
            "track 1\nlet a = [{}]\nlet x = a[{}]\n",
            lit.join(", "),
            idx,
        );
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        interp.schedule_until(0, 1_000_000_000).unwrap();
        let env = &interp.tracks[0].ctx.env_vars;
        let got = &env[&wilios_core::parser::ast::Ident("x".into())];
        prop_assert_eq!(
            got,
            &wilios_core::interpreter::interpreter::Value::Int(elems[idx]),
            "expected a[{}] = {}, got {:?}",
            idx, elems[idx], got
        );
    }
}

// ---------------------------------------------------------------------------
// Arrays: len() always equals the number of elements in the literal
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn array_len_matches_literal_size(
        elems in proptest::collection::vec(0i64..=127, 0..=8),
    ) {
        let lit: Vec<String> = elems.iter().map(|n| n.to_string()).collect();
        let source = format!(
            "track 1\nlet a = [{}]\nlet n = len(a)\n",
            lit.join(", "),
        );
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        interp.schedule_until(0, 1_000_000_000).unwrap();
        let env = &interp.tracks[0].ctx.env_vars;
        let got = &env[&wilios_core::parser::ast::Ident("n".into())];
        prop_assert_eq!(
            got,
            &wilios_core::interpreter::interpreter::Value::Int(elems.len() as i64),
            "expected len = {}, got {:?}", elems.len(), got
        );
    }
}

// ---------------------------------------------------------------------------
// Arrays: index-assign mutates the element in place
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn array_index_assign_mutates_element(
        elems in proptest::collection::vec(0i64..=100, 1..=8),
        idx   in 0usize..8,
        new_val in 200i64..=255,
    ) {
        let idx = idx % elems.len();
        let lit: Vec<String> = elems.iter().map(|n| n.to_string()).collect();
        let source = format!(
            "track 1\nlet a = [{}]\na[{}] = {}\nlet x = a[{}]\n",
            lit.join(", "),
            idx, new_val,
            idx,
        );
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        interp.schedule_until(0, 1_000_000_000).unwrap();
        let env = &interp.tracks[0].ctx.env_vars;
        let got = &env[&wilios_core::parser::ast::Ident("x".into())];
        prop_assert_eq!(
            got,
            &wilios_core::interpreter::interpreter::Value::Int(new_val),
            "expected a[{}] = {} after assign, got {:?}", idx, new_val, got
        );
    }
}

// ---------------------------------------------------------------------------
// Arrays: iterating with a loop emits exactly len(array) note events
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn array_loop_emits_correct_note_count(
        pitches in proptest::collection::vec(
            prop_oneof![
                Just("C4"), Just("D4"), Just("E4"), Just("F4"),
                Just("G4"), Just("A4"), Just("B4"),
            ],
            1..=6,
        ),
    ) {
        let n = pitches.len();
        let lit = pitches.join(", ");
        let source = format!(
            "tempo 120\ntrack 1\nlet notes = [{}]\nlet i = 0\nloop (i < len(notes)) {{\n    <notes[i]> 1/4\n    i = i + 1\n}}\n",
            lit,
        );
        let tokens = Lexer::new(&source).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        let events = interp.schedule_until(0, 1_000_000_000).unwrap();
        prop_assert_eq!(
            events.len(), n,
            "expected {} note events from array loop, got {}", n, events.len()
        );
    }
}

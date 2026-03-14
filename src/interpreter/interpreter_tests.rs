use super::{Interpreter, Value};
use crate::interpreter::event::EventKind;
use crate::lexer::Lexer;
use crate::parser::ast::{Pitch, Waveform};
use crate::parser::parser::Parser;

fn run(src: &str) {
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    Interpreter::new(program).unwrap().schedule_until(0, 1000000000000).unwrap();
}

#[test]
fn interpreter_variable_assign() {
    let mut l = Lexer::new(
        "
    let a = 1
    a = 2
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let mut interpreter = Interpreter::new(program).unwrap();
    interpreter.schedule_until(0, 1000000000000).unwrap();
}

#[test]
#[should_panic(expected = "Division by zero")]
fn interpreter_div_by_zero() {
    run("let x = 10 / 0");
}

#[test]
#[should_panic(expected = "Modulo by zero")]
fn interpreter_mod_by_zero() {
    run("let x = 10 % 0");
}

#[test]
#[should_panic(expected = "Duration division cannot be zero")]
fn interpreter_rest_division_by_zero() {
    run("rest 4/0");
}

// ---- Function tests ----

#[test]
fn interpreter_function_stmt_call() {
    // Function defined globally, called as a statement inside a track
    run("let f = func(x) { return x }\ntrack 1\nf(42)");
}

#[test]
fn interpreter_function_expr_call() {
    // Function call in expression assigns return value to variable
    run("let f = func(x) { return x }\nlet y = f(99)");
}

#[test]
fn interpreter_function_call_emits_note() {
    // Function that plays a note: the note should be emitted
    let src = "let play = func() { <C4> 1/4 }\ntrack 1\nplay()";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
}

// ---- Built-in function tests ----

#[test]
fn interpreter_builtin_rand_runs() {
    // rand(min, max) should not panic and program completes
    run("let n = rand(1, 10)\nlet m = rand(5, 5)");
}

#[test]
fn interpreter_builtin_transpose_up_one_octave() {
    // transpose(<C4>, 12) should produce C5 played as a note
    let src = "let p = transpose(<C4>, 12)\ntrack 1\n<p> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'C');
    assert_eq!(pitch.accidental, 0);
    assert_eq!(pitch.octave, 5);
}

#[test]
fn interpreter_builtin_transpose_down_one_octave() {
    // transpose(<C4>, -12) should produce C3
    let src = "let p = transpose(<C4>, -12)\ntrack 1\n<p> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'C');
    assert_eq!(pitch.accidental, 0);
    assert_eq!(pitch.octave, 3);
}

#[test]
fn interpreter_builtin_transpose_seventh() {
    // transpose(<C4>, 7) = G4 (perfect fifth / dominant)
    let src = "let p = transpose(<C4>, 7)\ntrack 1\n<p> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'G');
    assert_eq!(pitch.accidental, 0);
    assert_eq!(pitch.octave, 4);
}

#[test]
fn interpreter_builtin_transpose_chord() {
    // transpose(<C4, E4>, 12) should produce a chord at C5, E5
    let src = "let ch = transpose(<C4, E4>, 12)\ntrack 1\n<ch> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    // Chord of 2 notes → 2 note events
    assert_eq!(events.len(), 2);
    for ev in &events {
        let EventKind::Note { pitch, .. } = &ev.kind;
        assert_eq!(pitch.octave, 5);
    }
}

// ---- Synth param tests ----

#[test]
fn interpreter_wave_square_affects_event() {
    let src = "track 1\nwave square\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { waveform, .. } = &events[0].kind;
    assert_eq!(*waveform, Waveform::Square);
}

#[test]
fn interpreter_wave_saw_affects_event() {
    let src = "track 1\nwave saw\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { waveform, .. } = &events[0].kind;
    assert_eq!(*waveform, Waveform::Saw);
}

#[test]
fn interpreter_adsr_affects_event() {
    let src = "track 1\nattack 20\ndecay 50\nsustain 80\nrelease 200\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note {
        attack_ms,
        decay_ms,
        sustain_level,
        release_ms,
        ..
    } = &events[0].kind;
    assert_eq!(*attack_ms, 20.0);
    assert_eq!(*decay_ms, 50.0);
    assert!((sustain_level - 0.8).abs() < 1e-5);
    assert_eq!(*release_ms, 200.0);
}

#[test]
fn interpreter_pan_affects_event() {
    let src = "track 1\npan -64\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pan, .. } = &events[0].kind;
    assert_eq!(*pan, -64);
}

#[test]
fn interpreter_volume_affects_event() {
    let src = "track 1\nvolume 80\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { volume, .. } = &events[0].kind;
    assert_eq!(*volume, 80);
}

// ---- Multi-track tests ----

#[test]
fn interpreter_multi_track_emits_all() {
    // Both tracks should emit exactly one note each
    let src = "tempo 120\ntrack 1\n<C4> 1/4\ntrack 2\n<E4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 2);
    let track_ids: Vec<usize> = events.iter().map(|e| e.track).collect();
    assert!(track_ids.contains(&1));
    assert!(track_ids.contains(&2));
}

#[test]
fn interpreter_multi_track_state_isolation() {
    // wave square in track 1 must not bleed into track 2 (which stays sine)
    let src = "track 1\nwave square\n<C4> 1/4\ntrack 2\n<E4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 2);
    for ev in &events {
        let EventKind::Note { waveform, .. } = &ev.kind;
        if ev.track == 1 {
            assert_eq!(*waveform, Waveform::Square);
        } else {
            assert_eq!(*waveform, Waveform::Sine);
        }
    }
}

// ---- Control flow tests ----

#[test]
fn interpreter_loop_emits_correct_count() {
    // Loop 3 times → exactly 3 note events
    let src = "tempo 120\ntrack 1\nlet i = 0\nloop (i < 3) {\n    <C4> 1/4\n    i = i + 1\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 3);
}

#[test]
fn interpreter_if_true_branch_executes() {
    // Condition is true → C4 is emitted
    let src = "tempo 120\ntrack 1\nlet i = 0\nif (i == 0) {\n    <C4> 1/4\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'C');
}

#[test]
fn interpreter_if_else_false_branch_executes() {
    // Condition is false → else branch (E4) is emitted, not then-branch (C4)
    let src =
        "tempo 120\ntrack 1\nlet i = 1\nif (i == 0) {\n    <C4> 1/4\n} else {\n    <E4> 1/4\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'E');
}

// ---- Chord tests ----

#[test]
fn interpreter_chord_emits_all_pitches() {
    // A 3-note chord should emit 3 separate note events at the same time
    let src = "tempo 120\ntrack 1\n<C4, E4, G4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 3);
    // All events occur at time 0
    for ev in &events {
        assert_eq!(ev.at, 0);
    }
}

// ---- Timing tests ----

#[test]
fn interpreter_tempo_affects_event_timing() {
    // At 60 BPM a quarter note = 1000ms; at 120 BPM it's 500ms.
    // Tempo must be set inside the track to take effect (global-scope tempo
    // is not propagated to track contexts).
    let make = |bpm: usize| {
        let src = format!("track 1\ntempo {bpm}\n<C4> 1/4\n<E4> 1/4");
        let tokens = Lexer::new(&src).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        interp.schedule_until(0, 1_000_000_000).unwrap()
    };
    let slow = make(60);
    let fast = make(120);
    // Both produce 2 notes
    assert_eq!(slow.len(), 2);
    assert_eq!(fast.len(), 2);
    // Second note should start later at 60 BPM than at 120 BPM
    assert!(
        slow[1].at > fast[1].at,
        "60 BPM note should start later than 120 BPM note"
    );
}

// ---- FM block tests ----

#[test]
fn interpreter_fm_block_two_op_runs() {
    run(
        "track 1\nfm {\n    algorithm [2->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 3.0 }\n}\n<C4> 1/4",
    );
}

#[test]
fn interpreter_fm_block_emits_fm_config() {
    let src = "track 1\nfm {\n    algorithm [2->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 3.0 }\n}\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();

    assert_eq!(events.len(), 1);
    let EventKind::Note { fm_block, .. } = &events[0].kind;
    let block = fm_block.as_ref().expect("fm_block should be Some");
    assert_eq!(block.algorithm, vec![(2, 1)]);
    assert_eq!(block.ops.len(), 2);
    assert_eq!(block.ops[0].id, 1);
    assert_eq!(block.ops[0].ratio, 1.0);
    assert_eq!(block.ops[0].level, 1.0);
    assert_eq!(block.ops[1].id, 2);
    assert_eq!(block.ops[1].ratio, 2.0);
    assert_eq!(block.ops[1].level, 3.0);
}

#[test]
fn interpreter_fm_block_three_op_chain() {
    // op3 -> op2 -> op1 (series chain)
    run(
        "track 1\nfm {\n    algorithm [3->2, 2->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 2.0 }\n    op 3 { ratio 3.0  level 1.5 }\n}\n<C4> 1/4",
    );
}

#[test]
fn interpreter_fm_block_parallel_modulators() {
    // ops 2 and 3 both modulate op 1 independently
    run(
        "track 1\nfm {\n    algorithm [2->1, 3->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 2.0 }\n    op 3 { ratio 3.5  level 1.0 }\n}\n<C4> 1/4",
    );
}

#[test]
fn interpreter_fm_block_feedback() {
    // self-modulation (feedback)
    run("track 1\nfm {\n    algorithm [1->1]\n    op 1 { ratio 1.0  level 0.5 }\n}\n<C4> 1/4");
}

#[test]
fn interpreter_fm_block_per_op_adsr() {
    let src = "track 1\nfm {\n    algorithm [2->1]\n    op 1 { ratio 1.0  level 1.0  attack 5  decay 100  sustain 80  release 300 }\n    op 2 { ratio 2.0  level 3.0  attack 0  decay 50   sustain 60  release 200 }\n}\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();

    assert_eq!(events.len(), 1);
    let EventKind::Note { fm_block, .. } = &events[0].kind;
    let block = fm_block.as_ref().unwrap();
    assert_eq!(block.ops[0].attack_ms, 5.0);
    assert_eq!(block.ops[0].decay_ms, 100.0);
    assert_eq!(block.ops[0].sustain_level, 0.8); // 80 / 100
    assert_eq!(block.ops[0].release_ms, 300.0);
    assert_eq!(block.ops[1].decay_ms, 50.0);
}

#[test]
fn interpreter_fm_block_replaces_previous() {
    // A second fm block should replace the first for subsequent notes
    let src = "track 1\nfm {\n    algorithm [2->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 3.0 }\n}\n<C4> 1/4\nfm {\n    algorithm [3->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 3 { ratio 3.0  level 2.0 }\n}\n<E4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();

    assert_eq!(events.len(), 2);
    let EventKind::Note {
        fm_block: block1, ..
    } = &events[0].kind;
    let EventKind::Note {
        fm_block: block2, ..
    } = &events[1].kind;
    assert_eq!(block1.as_ref().unwrap().algorithm, vec![(2, 1)]);
    assert_eq!(block2.as_ref().unwrap().algorithm, vec![(3, 1)]);
}

#[test]
fn interpreter_legacy_fm_unaffected() {
    // fm_ratio / fm_depth still work, and fm_block should be None
    let src = "track 1\nfm_ratio 2.0\nfm_depth 1.5\n<C4> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();

    assert_eq!(events.len(), 1);
    let EventKind::Note {
        fm_block,
        fm_ratio,
        fm_depth,
        ..
    } = &events[0].kind;
    assert!(fm_block.is_none());
    assert_eq!(*fm_ratio, 2.0);
    assert_eq!(*fm_depth, 1.5);
}

// =========================================================
// ARRAY TESTS
// =========================================================

#[test]
fn interpreter_array_create_and_read() {
    let src = "track 1\nlet a = [10, 20, 30]\nlet x = a[1]";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(env[&crate::parser::ast::Ident("x".into())], Value::Int(20));
}

#[test]
fn interpreter_array_index_assign() {
    let src = "track 1\nlet a = [1, 2, 3]\na[0] = 99\nlet x = a[0]";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(env[&crate::parser::ast::Ident("x".into())], Value::Int(99));
}

#[test]
fn interpreter_array_len() {
    let src = "track 1\nlet a = [1, 2, 3]\nlet n = len(a)";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(env[&crate::parser::ast::Ident("n".into())], Value::Int(3));
}

#[test]
fn interpreter_array_empty_len() {
    let src = "track 1\nlet a = []\nlet n = len(a)";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(env[&crate::parser::ast::Ident("n".into())], Value::Int(0));
}

#[test]
fn interpreter_array_pitch_read() {
    let src = "track 1\nlet notes = [C4, E4, G4]\nlet p = notes[1]";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(
        env[&crate::parser::ast::Ident("p".into())],
        Value::Pitch(Pitch { letter: 'E', accidental: 0, octave: 4 })
    );
}

#[test]
fn interpreter_array_chord_read() {
    // Array of chords; read one and check it's a Chord value
    let src = "track 1\nlet chords = [<C4, E4>, <D4, F4>]\nlet c = chords[0]";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    interp.schedule_until(0, 1_000_000_000).unwrap();
    let env = interp.tracks[0].ctx.env_vars.clone();
    assert_eq!(
        env[&crate::parser::ast::Ident("c".into())],
        Value::Chord(vec![
            Pitch { letter: 'C', accidental: 0, octave: 4 },
            Pitch { letter: 'E', accidental: 0, octave: 4 },
        ])
    );
}

#[test]
fn interpreter_array_pitch_plays_note() {
    // <notes[0]> 1/4 should emit one note event for C4
    let src = "track 1\ntempo 120\nlet notes = [C4, E4, G4]\n<notes[0]> 1/4";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 1);
    let EventKind::Note { pitch, .. } = &events[0].kind;
    assert_eq!(pitch.letter, 'C');
    assert_eq!(pitch.octave, 4);
}

#[test]
fn interpreter_array_loop_over_pitches() {
    // Loop over 3-element pitch array, emitting each pitch
    let src = "track 1\ntempo 120\nlet notes = [C4, E4, G4]\nlet i = 0\nloop (i < len(notes)) {\n    <notes[i]> 1/4\n    i = i + 1\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let events = interp.schedule_until(0, 1_000_000_000).unwrap();
    assert_eq!(events.len(), 3);
}

#[test]
fn interpreter_array_out_of_bounds_error() {
    let src = "track 1\nlet a = [1, 2]\nlet x = a[5]";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let result = interp.schedule_until(0, 1_000_000_000);
    assert!(result.is_err());
}

#[test]
fn interpreter_array_index_assign_out_of_bounds_error() {
    let src = "track 1\nlet a = [1, 2]\na[5] = 99";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut interp = Interpreter::new(program).unwrap();
    let result = interp.schedule_until(0, 1_000_000_000);
    assert!(result.is_err());
}

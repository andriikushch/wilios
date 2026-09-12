//! End-to-end checks for the offline MIDI exporter: export a `.wilios` file and
//! parse the resulting `.mid` back with `midly`.

use std::path::PathBuf;

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use wilios_cli::midi::{MidiOpts, export_midi};
use wilios_cli::pipeline::load_interpreter;

fn write_program(name: &str, src: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, src).unwrap();
    path
}

fn out_path(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_file(&p);
    p
}

/// All (absolute tick, kind) pairs of a parsed track, tick from delta accumulation.
fn timeline(track: &[midly::TrackEvent]) -> Vec<(u32, TrackEventKind<'static>)> {
    let mut t = 0u32;
    track
        .iter()
        .map(|ev| {
            t += ev.delta.as_int();
            (t, ev.kind.to_static())
        })
        .collect()
}

#[test]
fn exports_notes_with_musical_ticks_and_a_tempo_map() {
    let src = write_program(
        "midi_basic.wilios",
        "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\n",
    );
    let out = out_path("midi_basic.mid");

    let interp = load_interpreter(&src).expect("load");
    export_midi(
        interp,
        MidiOpts {
            out: out.clone(),
            duration: Some(2.0),
            max_secs: 5.0,
        },
    )
    .expect("export");

    let bytes = std::fs::read(&out).expect("read mid");
    let smf = Smf::parse(&bytes).expect("parse mid");

    assert_eq!(smf.header.format, Format::Parallel);
    assert_eq!(smf.header.timing, Timing::Metrical(480.into()));
    // track 0 (tempo map) + one track per wilios track.
    assert_eq!(smf.tracks.len(), 2);

    // Tempo map: 120 BPM == 500_000 us per quarter note, at tick 0.
    let tempo = timeline(&smf.tracks[0]);
    assert!(tempo.iter().any(|(tick, kind)| *tick == 0
        && matches!(kind, TrackEventKind::Meta(MetaMessage::Tempo(us)) if us.as_int() == 500_000)));

    // Note track: C4 (60) at tick 0, off at 480 (a 1/4 note = one quarter =
    // 480 ticks), then E4 (64) at 480.
    let notes = timeline(&smf.tracks[1]);
    let note_msgs: Vec<_> = notes
        .iter()
        .filter_map(|(tick, kind)| match kind {
            TrackEventKind::Midi { message, .. } => Some((*tick, *message)),
            _ => None,
        })
        .collect();
    assert_eq!(note_msgs[0].0, 0);
    assert!(matches!(note_msgs[0].1, MidiMessage::NoteOn { key, .. } if key == 60));
    assert!(matches!(
        note_msgs[1],
        (480, MidiMessage::NoteOff { key, .. }) if key == 60
    ));
    assert!(matches!(
        note_msgs[2],
        (480, MidiMessage::NoteOn { key, .. }) if key == 64
    ));

    assert!(!out.with_extension("mid.partial").exists());
}

#[test]
fn mid_piece_tempo_change_becomes_a_second_tempo_meta() {
    let src = write_program(
        "midi_tempo.wilios",
        "track 1\ntempo 120\n<C4> 1/4\ntempo 60\n<C4> 1/4\n",
    );
    let out = out_path("midi_tempo.mid");

    let interp = load_interpreter(&src).expect("load");
    export_midi(
        interp,
        MidiOpts {
            out: out.clone(),
            duration: Some(3.0),
            max_secs: 5.0,
        },
    )
    .expect("export");

    let bytes = std::fs::read(&out).expect("read mid");
    let smf = Smf::parse(&bytes).expect("parse mid");

    let tempos: Vec<(u32, u32)> = timeline(&smf.tracks[0])
        .into_iter()
        .filter_map(|(tick, kind)| match kind {
            TrackEventKind::Meta(MetaMessage::Tempo(us)) => Some((tick, us.as_int())),
            _ => None,
        })
        .collect();
    assert_eq!(tempos.len(), 2);
    assert_eq!(tempos[0], (0, 500_000)); // 120 BPM
    assert_eq!(tempos[1], (480, 1_000_000)); // 60 BPM, after one quarter note
}

#[test]
fn endless_loop_without_duration_errors_and_writes_no_file() {
    let src = write_program(
        "midi_endless.wilios",
        "track 1\nloop (true) {\n  <C4> 1/4\n}\n",
    );
    let out = out_path("midi_endless.mid");

    let interp = load_interpreter(&src).expect("load");
    let err = export_midi(
        interp,
        MidiOpts {
            out: out.clone(),
            duration: None,
            max_secs: 1.0,
        },
    )
    .expect_err("must error");
    assert!(err.contains("--duration"), "unexpected error: {err}");
    assert!(!out.exists());
    assert!(!out.with_extension("mid.partial").exists());

    // With a bound it succeeds.
    let interp = load_interpreter(&src).expect("load");
    export_midi(
        interp,
        MidiOpts {
            out: out.clone(),
            duration: Some(2.0),
            max_secs: 5.0,
        },
    )
    .expect("bounded export");
    assert!(out.exists());
}

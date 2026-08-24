use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use wilios_core::interpreter::event::EventKind;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::interpreter::pitch::{Accidental, Pitch, PitchName, note_frequency};
use wilios_core::lexer::Lexer;
use wilios_core::parser::parser::Parser;
use wilios_synth::{Mixer, Voice};

fn main() {
    // stderr, not stdout: this binary's logic is the template for wilios-mcp,
    // which will reserve stdout for its own protocol framing. RUST_LOG (e.g.
    // `RUST_LOG=debug`) controls verbosity; defaults to info-level.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    let voices = Arc::new(Mutex::new(Vec::<Voice>::new()));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        tracing::error!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let file_path = std::path::PathBuf::from(&args[1])
        .canonicalize()
        .unwrap_or_else(|e| {
            tracing::error!("Error resolving '{}': {}", args[1], e);
            std::process::exit(1);
        });
    let source = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
        tracing::error!("Error reading '{}': {}", args[1], e);
        std::process::exit(1);
    });
    let mut l = Lexer::new(&source);

    let tokens = l.lex().unwrap_or_else(|e| {
        tracing::error!("error: {}", e);
        std::process::exit(1);
    });
    let base_dir = file_path.parent().map(|p| p.to_path_buf());
    let loaded = std::collections::HashSet::from([file_path]);
    let program = Parser::new_with_context(tokens, base_dir, loaded)
        .parse()
        .unwrap_or_else(|e| {
            tracing::error!("parse error: {}", e);
            std::process::exit(1);
        });

    let interpreter = Interpreter::new(program).expect("runtime error in global scope");

    let voices_cb = voices.clone();
    let mut interpreter_cb = interpreter.clone();
    let _start_time = Instant::now();

    let finished = Arc::new(AtomicBool::new(false));
    let finished_cb = finished.clone();

    let mut mixer = Mixer::new(sample_rate, channels);
    let mut sample_counter: u64 = 0;

    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let mut voices_lock = voices_cb.lock().unwrap();

                // Schedule all events for this buffer in one call instead of per-sample.
                // data.len() == frames * channels; we need frame count for correct timing.
                let buffer_frames = (data.len() / channels) as u64;
                let buffer_end_ms =
                    ((sample_counter + buffer_frames) as f32 / sample_rate * 1000.0) as u64;
                let events = interpreter_cb
                    .schedule_until(0, buffer_end_ms)
                    .unwrap_or_default();
                for ev in events {
                    let EventKind::Note {
                        pitch,
                        duration,
                        volume,
                        waveform,
                        attack_ms,
                        decay_ms,
                        sustain_level,
                        release_ms,
                        fm_ratio,
                        fm_depth,
                        fm_block,
                        ..
                    } = ev.kind;
                    let freq = note_frequency(
                        Pitch {
                            name: PitchName::from_string(pitch.letter),
                            accidental: Accidental::from_int(pitch.accidental),
                        },
                        pitch.octave as u8,
                    );
                    voices_lock.push(Voice::new(
                        freq,
                        sample_rate,
                        volume as f32 / 127.0,
                        duration,
                        waveform,
                        attack_ms,
                        decay_ms,
                        sustain_level,
                        release_ms,
                        fm_ratio,
                        fm_depth,
                        fm_block,
                    ));
                }

                mixer.process(&mut voices_lock, data);
                sample_counter += buffer_frames;

                if voices_lock.is_empty() && interpreter_cb.all_tracks_finished() {
                    finished_cb.store(true, Ordering::Relaxed);
                }
            },
            |err| tracing::error!("Audio error: {:?}", err),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    tracing::info!("Playing DSL program… press Enter to quit early, or wait for it to finish");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
        let _ = tx.send(());
    });
    loop {
        if finished.load(Ordering::Relaxed) || rx.try_recv().is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

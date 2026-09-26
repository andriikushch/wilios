//! Live playback: the cpal output stream as one consumer of the core pipeline.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use wilios_core::interpreter::interpreter::Interpreter;
use wilios_synth::{Mixer, Voice};

use crate::voices::VoiceScheduler;

/// Open the default output device and play `interp` in real time, returning when
/// the piece finishes on its own or the user presses Enter.
pub fn play(interp: Interpreter) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No output device available")?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("No default output config: {e}"))?;
    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    let voices = Arc::new(Mutex::new(Vec::<Voice>::new()));
    let voices_cb = voices.clone();
    let mut interpreter_cb = interp;

    let finished = Arc::new(AtomicBool::new(false));
    let finished_cb = finished.clone();

    let mut mixer = Mixer::new(sample_rate, channels);
    let mut sample_counter: u64 = 0;
    let mut scheduler = VoiceScheduler::new();

    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let mut voices_lock = voices_cb.lock().unwrap();

                // Schedule all events for this buffer in one call instead of per-sample.
                // data.len() == frames * channels; we need frame count for correct timing.
                let buffer_frames = (data.len() / channels) as u64;
                // f64 to match the offline renderer and stay exact past ~6 min of playback.
                let ms_at = |frames: u64| (frames as f64 / sample_rate as f64 * 1000.0) as u64;
                let buffer_start_ms = ms_at(sample_counter);
                let buffer_end_ms = ms_at(sample_counter + buffer_frames);
                // Live playback should not abort mid-stream on a scheduler error.
                let _ = scheduler.drain_new_voices(
                    &mut interpreter_cb,
                    buffer_start_ms,
                    buffer_end_ms,
                    sample_rate,
                    &mut voices_lock,
                );

                mixer.process(&mut voices_lock, data);
                sample_counter += buffer_frames;

                if voices_lock.is_empty() && interpreter_cb.all_tracks_finished() {
                    finished_cb.store(true, Ordering::Relaxed);
                }
            },
            |err| tracing::error!("Audio error: {:?}", err),
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {e}"))?;

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
    Ok(())
}

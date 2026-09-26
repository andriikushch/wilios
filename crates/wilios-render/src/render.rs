//! Offline render: run the pipeline straight to a WAV file (or an in-memory
//! sample buffer), no audio device.
//!
//! Output is 16-bit signed PCM, stereo, matching the live cpal path (the
//! [`Mixer`] writes the same mono sample to every channel).

use std::path::{Path, PathBuf};

use wilios_core::interpreter::interpreter::Interpreter;
use wilios_synth::{Mixer, Voice};

use crate::voices::VoiceScheduler;

pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;
/// When no `--duration` is given, a piece that hasn't finished by this many
/// seconds is treated as non-terminating and the render fails with no file.
pub const DEFAULT_MAX_RENDER_SECS: f32 = 600.0;

const CHANNELS: u16 = 2;
const FRAMES: usize = 1024;

pub struct RenderOpts {
    pub out: PathBuf,
    pub sample_rate: u32,
    /// `Some(secs)` renders exactly that long (silence-padded / truncated as
    /// needed); `None` renders until the piece ends, bounded by
    /// [`RenderOpts::max_render_secs`].
    pub duration: Option<f32>,
    pub max_render_secs: f32,
}

impl RenderOpts {
    pub fn new(out: PathBuf) -> Self {
        Self {
            out,
            sample_rate: DEFAULT_SAMPLE_RATE,
            duration: None,
            max_render_secs: DEFAULT_MAX_RENDER_SECS,
        }
    }
}

/// Interleaved stereo f32 from an offline render, plus whether the piece ended
/// on its own. `finished == false` means rendering stopped at the
/// `max_render_secs` cap (or the fixed `duration`) with tracks still active —
/// an endless loop, or simply a piece longer than the bound.
pub struct RenderSamples {
    pub interleaved: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
    pub finished: bool,
}

/// Progress ping from an offline render — one per mixer buffer, plus a final one
/// when the loop ends so a UI always lands on the true end state.
///
/// `wilios-render` never renders this itself; the caller passes an `FnMut` and
/// decides what (if anything) to draw. The CLI turns it into a stderr bar; the
/// MCP server ignores it.
pub struct RenderProgress {
    /// Sample-frames rendered so far.
    pub frames_done: u64,
    /// Exact total frames when a fixed `--duration` was given; `None` for an
    /// open-ended render, where only the `max_render_secs` cap is known — show
    /// elapsed seconds, not a percentage.
    pub frames_total: Option<u64>,
    pub sample_rate: u32,
    /// `true` for the single ping emitted after the render loop ends — the
    /// render is complete regardless of what `fraction()` says (an open-ended
    /// render has no fraction). A terminal UI should finish its line here.
    pub done: bool,
}

impl RenderProgress {
    /// Completed fraction in `0.0..=1.0`, or `None` for an open-ended render.
    pub fn fraction(&self) -> Option<f64> {
        match self.frames_total {
            Some(total) if total > 0 => {
                Some((self.frames_done as f64 / total as f64).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// Seconds of audio rendered so far.
    pub fn seconds_done(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames_done as f64 / self.sample_rate as f64
        }
    }

    /// Target length in seconds, or `None` for an open-ended render.
    pub fn seconds_total(&self) -> Option<f64> {
        match self.frames_total {
            Some(total) if self.sample_rate != 0 => Some(total as f64 / self.sample_rate as f64),
            _ => None,
        }
    }
}

impl RenderSamples {
    /// Number of sample frames (interleaved length / channels).
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.interleaved.len() / self.channels as usize
        }
    }

    /// Rendered length in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            0
        } else {
            (self.frames() as f64 / self.sample_rate as f64 * 1000.0).round() as u64
        }
    }
}

/// Drive `interp` through the mixer and collect interleaved stereo f32 — no
/// file. A non-terminating piece comes back with `finished: false` and samples
/// truncated at `opts.max_render_secs` (or `opts.duration`); it is **not** an
/// error. This is the shared core of [`render`] and the `render` MCP tool.
pub fn render_to_samples(interp: Interpreter, opts: &RenderOpts) -> Result<RenderSamples, String> {
    render_to_samples_with_progress(interp, opts, &mut |_| {})
}

/// [`render_to_samples`] with a progress callback invoked once per mixer buffer
/// (~43×/s at 44.1 kHz) and once more when the render loop ends. Callers that
/// want to throttle should do so in the closure; the render loop does not.
pub fn render_to_samples_with_progress(
    mut interp: Interpreter,
    opts: &RenderOpts,
    on_progress: &mut dyn FnMut(RenderProgress),
) -> Result<RenderSamples, String> {
    if opts.sample_rate == 0 {
        return Err("sample rate must be greater than 0".into());
    }
    let sr = opts.sample_rate as f32;

    let target_samples: Option<u64> = opts
        .duration
        .map(|d| (d as f64 * opts.sample_rate as f64).round() as u64);
    let cap_samples: u64 = (opts.max_render_secs as f64 * opts.sample_rate as f64).round() as u64;

    let mut mixer = Mixer::new(sr, CHANNELS as usize);
    let mut scratch = vec![0f32; FRAMES * CHANNELS as usize];
    let mut voices: Vec<Voice> = Vec::new();
    let mut scheduler = VoiceScheduler::new();
    let mut samples: Vec<f32> = Vec::new();
    let mut sample_counter: u64 = 0;
    let finished;

    loop {
        // Trim the final buffer so a fixed --duration lands on the exact sample.
        let frames_this_buffer = match target_samples {
            Some(target) if sample_counter + FRAMES as u64 >= target => {
                (target - sample_counter) as usize
            }
            _ => FRAMES,
        };

        let ms_at = |frames: u64| (frames as f64 / opts.sample_rate as f64 * 1000.0) as u64;
        let buffer_start_ms = ms_at(sample_counter);
        let buffer_end_ms = ms_at(sample_counter + frames_this_buffer as u64);
        scheduler
            .drain_new_voices(&mut interp, buffer_start_ms, buffer_end_ms, sr, &mut voices)
            .map_err(|e| e.0)?;

        for s in scratch.iter_mut() {
            *s = 0.0;
        }
        mixer.process(&mut voices, &mut scratch);

        let keep = frames_this_buffer * CHANNELS as usize;
        samples.extend_from_slice(&scratch[..keep]);
        sample_counter += frames_this_buffer as u64;

        on_progress(RenderProgress {
            frames_done: sample_counter,
            frames_total: target_samples,
            sample_rate: opts.sample_rate,
            done: false,
        });

        match target_samples {
            Some(target) => {
                if sample_counter >= target {
                    finished = interp.all_tracks_finished() && voices.is_empty();
                    break;
                }
            }
            None => {
                if interp.all_tracks_finished() && voices.is_empty() {
                    finished = true;
                    break;
                }
                if sample_counter >= cap_samples {
                    finished = false;
                    break;
                }
            }
        }
    }

    // A forced final ping (the per-buffer one above may have been throttled away
    // by the caller) so a UI can settle on the true end state.
    on_progress(RenderProgress {
        frames_done: sample_counter,
        frames_total: target_samples,
        sample_rate: opts.sample_rate,
        done: true,
    });

    Ok(RenderSamples {
        interleaved: samples,
        channels: CHANNELS,
        sample_rate: opts.sample_rate,
        finished,
    })
}

/// Encode a [`RenderSamples`] as a 16-bit stereo WAV in memory, byte-identical
/// to what [`render`] writes to disk.
pub fn encode_wav_bytes(s: &RenderSamples) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: s.channels,
        sample_rate: s.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut buf);
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("Error creating WAV: {e}"))?;
        for &v in &s.interleaved {
            let q = (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(q)
                .map_err(|e| format!("Error writing samples: {e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Error finalizing WAV: {e}"))?;
    }
    Ok(buf)
}

/// Render `interp` to `opts.out`. Writes to a `<out>.partial` sibling and renames
/// it into place only on success, so no output file is left behind on error.
pub fn render(interp: Interpreter, opts: RenderOpts) -> Result<(), String> {
    render_with_progress(interp, opts, &mut |_| {})
}

/// [`render`] with a progress callback threaded into the synthesis loop (see
/// [`render_to_samples_with_progress`]). The WAV encode and file write that
/// follow are fast and not reported.
pub fn render_with_progress(
    interp: Interpreter,
    opts: RenderOpts,
    on_progress: &mut dyn FnMut(RenderProgress),
) -> Result<(), String> {
    let samples = render_to_samples_with_progress(interp, &opts, on_progress)?;

    if opts.duration.is_none() && !samples.finished {
        return Err(format!(
            "piece did not finish within {:.0}s and no --duration was given — \
             it likely contains an endless loop (`loop (true) {{ … }}`) or a \
             note shorter than its attack that never releases. \
             Re-run with --duration <seconds>.",
            opts.max_render_secs
        ));
    }

    let bytes = encode_wav_bytes(&samples)?;

    let tmp_path = partial_path(&opts.out);
    std::fs::write(&tmp_path, &bytes)
        .map_err(|e| format!("Error creating '{}': {e}", tmp_path.display()))?;
    if let Err(e) = std::fs::rename(&tmp_path, &opts.out) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("Error moving output into place: {e}"));
    }

    let frames = samples.frames() as u64;
    tracing::info!(
        "Rendered {} ({:.2}s, {} frames @ {} Hz stereo 16-bit)",
        opts.out.display(),
        frames as f64 / opts.sample_rate as f64,
        frames,
        opts.sample_rate
    );
    Ok(())
}

pub fn partial_path(out: &Path) -> PathBuf {
    let mut name = out
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| "out.wav".into());
    name.push(".partial");
    out.with_file_name(name)
}

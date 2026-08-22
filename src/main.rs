use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use wilios::interpreter::event::{EventKind, FmBlockConfig, FmOpConfig};
use wilios::interpreter::interpreter::Interpreter;
use wilios::interpreter::pitch::{Accidental, Pitch, PitchName, note_frequency};
use wilios::lexer::Lexer;
use wilios::parser::ast::Waveform;
use wilios::parser::parser::Parser;
// ======================= AUDIO VOICE =======================

#[derive(Clone, Copy)]
enum EnvState {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

struct Envelope {
    value: f32,
    sustain_level: f32,
    attack_inc: f32,
    decay_inc: f32,
    release_inc: f32,
    state: EnvState,
}

impl Envelope {
    fn new(
        sample_rate: f32,
        attack_ms: f32,
        decay_ms: f32,
        sustain_level: f32,
        release_ms: f32,
    ) -> Self {
        let attack_inc = if attack_ms > 0.0 {
            1.0 / (sample_rate * attack_ms / 1000.0)
        } else {
            1.0
        };
        let decay_inc = if decay_ms > 0.0 {
            (1.0 - sustain_level) / (sample_rate * decay_ms / 1000.0)
        } else {
            1.0
        };
        let release_inc = if release_ms > 0.0 {
            sustain_level / (sample_rate * release_ms / 1000.0)
        } else {
            1.0
        };
        Self {
            value: 0.0,
            sustain_level,
            attack_inc,
            decay_inc,
            release_inc,
            state: EnvState::Attack,
        }
    }

    fn next(&mut self) -> f32 {
        match self.state {
            EnvState::Attack => {
                self.value += self.attack_inc;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.state = if self.sustain_level < 1.0 {
                        EnvState::Decay
                    } else {
                        EnvState::Sustain
                    };
                }
            }
            EnvState::Decay => {
                self.value -= self.decay_inc;
                if self.value <= self.sustain_level {
                    self.value = self.sustain_level;
                    self.state = EnvState::Sustain;
                }
            }
            EnvState::Release => {
                self.value -= self.release_inc;
                if self.value <= 0.0 {
                    self.value = 0.0;
                    self.state = EnvState::Off;
                }
            }
            _ => {}
        }
        self.value
    }

    fn note_off(&mut self, sample_rate: f32, release_ms: f32) {
        // Recompute release_inc from current value so release always completes in time
        self.release_inc = if release_ms > 0.0 {
            self.value / (sample_rate * release_ms / 1000.0)
        } else {
            1.0
        };
        self.state = EnvState::Release;
    }

    fn finished(&self) -> bool {
        matches!(self.state, EnvState::Off)
    }
}

/// Standalone waveform sampler — avoids borrowing the Voice struct.
fn sample_waveform(wave: &Waveform, phase: f32) -> f32 {
    match wave {
        Waveform::Sine => (phase * 2.0 * PI).sin(),
        Waveform::Square => {
            if (phase % 1.0) < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Saw => (phase % 1.0) * 2.0 - 1.0,
        Waveform::Triangle => {
            let p = phase % 1.0;
            if p < 0.5 {
                p * 4.0 - 1.0
            } else {
                (1.0 - p) * 4.0 - 1.0
            }
        }
    }
}

/// State for one FM operator in multi-op mode.
struct OpState {
    phase: f32,
    phase_inc: f32, // freq * ratio / sample_rate
    level: f32,     // modulation depth (for modulators) or output amplitude (for carriers)
    wave: Waveform,
    env: Envelope,
    release_ms: f32,
    last_output: f32, // raw waveform output of previous sample (used for modulation/feedback)
}

struct Voice {
    // Legacy 2-op fields (used when op_states is None)
    phase: f32,
    phase_inc: f32,
    mod_phase: f32,
    mod_phase_inc: f32,
    fm_depth: f32,
    waveform: Waveform,
    env: Envelope,
    release_ms: f32,

    // Common
    volume: f32,
    remaining_samples: u64,
    sample_rate: f32,

    // Multi-op FM (Some = use multi-op path; None = use legacy path above)
    op_states: Option<Vec<OpState>>,
    // algorithm_indices[i] = (src_idx, dst_idx) as indices into op_states
    algorithm_indices: Vec<(usize, usize)>,
    // indices into op_states of operators that contribute to the audio output
    carrier_indices: Vec<usize>,
    // processing order: indices into op_states, modulators before their targets
    process_order: Vec<usize>,
}

impl Voice {
    #[allow(clippy::too_many_arguments)]
    fn new(
        freq: f32,
        sample_rate: f32,
        volume: f32,
        duration_ms: u64,
        waveform: Waveform,
        attack_ms: f32,
        decay_ms: f32,
        sustain_level: f32,
        release_ms: f32,
        fm_ratio: f32,
        fm_depth: f32,
        fm_block: Option<FmBlockConfig>,
    ) -> Self {
        let remaining_samples = (duration_ms as f32 / 1000.0 * sample_rate) as u64;

        if let Some(cfg) = fm_block {
            // ---- Multi-operator FM path ----
            let mut ops: Vec<OpState> = cfg
                .ops
                .iter()
                .map(|op: &FmOpConfig| OpState {
                    phase: 0.0,
                    phase_inc: freq * op.ratio / sample_rate,
                    level: op.level,
                    wave: op.wave.clone(),
                    env: Envelope::new(
                        sample_rate,
                        op.attack_ms,
                        op.decay_ms,
                        op.sustain_level,
                        op.release_ms,
                    ),
                    release_ms: op.release_ms,
                    last_output: 0.0,
                })
                .collect();

            // Build id -> index map
            let id_to_idx: std::collections::HashMap<usize, usize> = cfg
                .ops
                .iter()
                .enumerate()
                .map(|(i, op)| (op.id, i))
                .collect();

            // Convert algorithm from (id, id) to (idx, idx)
            let algorithm_indices: Vec<(usize, usize)> = cfg
                .algorithm
                .iter()
                .filter_map(|(src_id, dst_id)| {
                    let s = id_to_idx.get(src_id)?;
                    let d = id_to_idx.get(dst_id)?;
                    Some((*s, *d))
                })
                .collect();

            // Carrier indices: ops that don't appear as a source in the algorithm.
            // If every op is a source, fall back to the first op (index 0).
            let source_indices: std::collections::HashSet<usize> =
                algorithm_indices.iter().map(|(s, _)| *s).collect();
            let mut carrier_indices: Vec<usize> = (0..ops.len())
                .filter(|i| !source_indices.contains(i))
                .collect();
            if carrier_indices.is_empty() {
                carrier_indices.push(0);
            }

            // Topological sort (Kahn's algorithm). Cycles get one-sample-delay feedback.
            let n = ops.len();
            let mut in_degree = vec![0usize; n];
            for &(_, d) in &algorithm_indices {
                in_degree[d] += 1;
            }
            let mut queue: std::collections::VecDeque<usize> =
                (0..n).filter(|&i| in_degree[i] == 0).collect();
            let mut process_order: Vec<usize> = Vec::with_capacity(n);
            while let Some(node) = queue.pop_front() {
                process_order.push(node);
                for &(s, d) in &algorithm_indices {
                    if s == node {
                        in_degree[d] -= 1;
                        if in_degree[d] == 0 {
                            queue.push_back(d);
                        }
                    }
                }
            }
            // Append any remaining (cycle members) in ascending index order
            for i in 0..n {
                if !process_order.contains(&i) {
                    process_order.push(i);
                }
            }

            // Silence unused legacy fields
            let _ = &mut ops; // ensure initialized before move
            Self {
                phase: 0.0,
                phase_inc: 0.0,
                mod_phase: 0.0,
                mod_phase_inc: 0.0,
                fm_depth: 0.0,
                waveform: Waveform::Sine,
                env: Envelope::new(sample_rate, 0.0, 0.0, 1.0, 0.0),
                release_ms: 0.0,
                volume,
                remaining_samples,
                sample_rate,
                op_states: Some(ops),
                algorithm_indices,
                carrier_indices,
                process_order,
            }
        } else {
            // ---- Legacy 2-op path ----
            Self {
                phase: 0.0,
                phase_inc: freq / sample_rate,
                mod_phase: 0.0,
                mod_phase_inc: freq * fm_ratio / sample_rate,
                fm_depth,
                waveform,
                env: Envelope::new(sample_rate, attack_ms, decay_ms, sustain_level, release_ms),
                release_ms,
                volume,
                remaining_samples,
                sample_rate,
                op_states: None,
                algorithm_indices: Vec::new(),
                carrier_indices: Vec::new(),
                process_order: Vec::new(),
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        if let Some(ops) = &mut self.op_states {
            // ---- Multi-operator FM synthesis ----
            let n = ops.len();

            // Snapshot levels (avoids double-borrow when reading src level while mutating dst)
            let levels: Vec<f32> = ops.iter().map(|o| o.level).collect();
            // Start with previous-sample outputs (provides feedback / initial values)
            let mut current_outputs: Vec<f32> = ops.iter().map(|o| o.last_output).collect();
            let mut env_vals: Vec<f32> = vec![0.0; n];

            let process_order = self.process_order.clone();
            let algorithm_indices = self.algorithm_indices.clone();

            for &idx in &process_order {
                // Sum modulation from all ops that target this op.
                // Apply the source op's envelope so modulation depth tracks its ADSR over time.
                // Self-feedback (src == idx) uses 1.0 because env_vals[idx] isn't set yet.
                let modulation: f32 = algorithm_indices
                    .iter()
                    .filter(|(_, dst)| *dst == idx)
                    .map(|(src, _)| {
                        let env = if *src == idx { 1.0 } else { env_vals[*src] };
                        current_outputs[*src] * env * levels[*src]
                    })
                    .sum();

                let raw = sample_waveform(&ops[idx].wave, ops[idx].phase + modulation);
                current_outputs[idx] = raw;
                env_vals[idx] = ops[idx].env.next();

                ops[idx].phase += ops[idx].phase_inc;
                if ops[idx].phase >= 1.0 {
                    ops[idx].phase -= 1.0;
                }
            }

            // Store outputs for next sample (feedback)
            for i in 0..n {
                ops[i].last_output = current_outputs[i];
            }

            // Note-off handling
            if self.remaining_samples > 0 {
                self.remaining_samples -= 1;
                if self.remaining_samples == 0 {
                    let sr = self.sample_rate;
                    for op in ops.iter_mut() {
                        op.env.note_off(sr, op.release_ms);
                    }
                }
            }

            // Sum carrier outputs
            let carrier_indices = self.carrier_indices.clone();
            let audio: f32 = carrier_indices
                .iter()
                .map(|&i| current_outputs[i] * env_vals[i] * levels[i])
                .sum();

            audio * self.volume
        } else {
            // ---- Legacy 2-op FM synthesis ----
            let mod_signal = (self.mod_phase * 2.0 * PI).sin();
            let carrier_phase = self.phase + self.fm_depth * mod_signal;

            let s = sample_waveform(&self.waveform, carrier_phase);

            self.phase += self.phase_inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.mod_phase += self.mod_phase_inc;
            if self.mod_phase >= 1.0 {
                self.mod_phase -= 1.0;
            }

            if self.remaining_samples > 0 {
                self.remaining_samples -= 1;
                if self.remaining_samples == 0
                    && matches!(self.env.state, EnvState::Sustain | EnvState::Decay)
                {
                    self.env.note_off(self.sample_rate, self.release_ms);
                }
            }

            s * self.env.next() * self.volume
        }
    }

    fn finished(&self) -> bool {
        if let Some(ops) = &self.op_states {
            self.remaining_samples == 0 && ops.iter().all(|o| o.env.finished())
        } else {
            self.remaining_samples == 0 && self.env.finished()
        }
    }
}

const MASTER_GAIN: f32 = 0.3;

// ======================= MAIN AUDIO LOOP =======================

fn main() {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    let voices = Arc::new(Mutex::new(Vec::<Voice>::new()));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let file_path = std::path::PathBuf::from(&args[1])
        .canonicalize()
        .unwrap_or_else(|e| {
            eprintln!("Error resolving '{}': {}", args[1], e);
            std::process::exit(1);
        });
    let source = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
        eprintln!("Error reading '{}': {}", args[1], e);
        std::process::exit(1);
    });
    let mut l = Lexer::new(&source);

    let tokens = l.lex().unwrap_or_else(|e| {
        eprintln!("error: {}", e);
        std::process::exit(1);
    });
    let base_dir = file_path.parent().map(|p| p.to_path_buf());
    let loaded = std::collections::HashSet::from([file_path]);
    let program = Parser::new_with_context(tokens, base_dir, loaded)
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("parse error: {}", e);
            std::process::exit(1);
        });

    // 1️⃣ Create interpreter
    let interpreter = Interpreter::new(program).expect("runtime error in global scope");

    let voices_cb = voices.clone();
    let mut interpreter_cb = interpreter.clone();
    let _start_time = Instant::now();

    // Peak limiter: instant attack, ~100 ms release
    let limiter_release = (-1.0_f32 / (sample_rate * 0.1)).exp();
    let mut peak_env: f32 = 0.0_f32;
    let mut sample_counter: u64 = 0;

    let stream = device
        .build_output_stream(
            &config.into(),
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

                // Per-frame synthesis: advance voices once per frame, write to all channels.
                // Iterating over individual samples would call next_sample() `channels` times
                // per frame, running every voice at `channels`× the correct frequency.
                for frame in data.chunks_mut(channels) {
                    let mut mix = 0.0f32;
                    for v in voices_lock.iter_mut() {
                        mix += v.next_sample();
                    }
                    let pre = mix * MASTER_GAIN;
                    let abs_pre = pre.abs();
                    if abs_pre > peak_env {
                        peak_env = abs_pre; // instant attack
                    } else {
                        peak_env *= limiter_release; // ~100 ms release
                    }
                    let gain = if peak_env > 1.0 { 1.0 / peak_env } else { 1.0 };
                    let x = pre * gain;
                    // Soft-knee tanh saturation: linear below 0.8, smooth rolloff above
                    const KNEE: f32 = 0.8_f32;
                    let abs_x = x.abs();
                    let out = if abs_x < KNEE {
                        x
                    } else {
                        let headroom = 1.0_f32 - KNEE;
                        let excess = (abs_x - KNEE) / headroom;
                        x.signum() * (KNEE + headroom * excess.tanh())
                    };
                    for ch in frame.iter_mut() {
                        *ch = out;
                    }
                    sample_counter += 1;
                }
                voices_lock.retain(|v| !v.finished());
            },
            |err| eprintln!("Audio error: {:?}", err),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    println!("Playing DSL program… press Enter to quit");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

use std::f32::consts::PI;

use wilios_core::interpreter::event::{FmBlockConfig, FmOpConfig};
use wilios_core::parser::ast::Waveform;

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

pub struct Voice {
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
    pub fn new(
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

    pub fn next_sample(&mut self) -> f32 {
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

    pub fn finished(&self) -> bool {
        if let Some(ops) = &self.op_states {
            self.remaining_samples == 0 && ops.iter().all(|o| o.env.finished())
        } else {
            self.remaining_samples == 0 && self.env.finished()
        }
    }
}

const MASTER_GAIN: f32 = 0.3;

/// Mixes active voices into an output buffer, applying a peak limiter and
/// soft-knee tanh saturation. Owns the limiter's running peak envelope so
/// state persists correctly across successive audio-callback buffers.
pub struct Mixer {
    channels: usize,
    limiter_release: f32,
    peak_env: f32,
}

impl Mixer {
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        // Peak limiter: instant attack, ~100 ms release
        let limiter_release = (-1.0_f32 / (sample_rate * 0.1)).exp();
        Self {
            channels,
            limiter_release,
            peak_env: 0.0,
        }
    }

    /// Advances `voices` by one buffer's worth of samples, writing the mixed,
    /// limited, saturated output into `data` (interleaved by `channels`), and
    /// drops any voices that finished during this buffer.
    pub fn process(&mut self, voices: &mut Vec<Voice>, data: &mut [f32]) {
        for frame in data.chunks_mut(self.channels) {
            let mut mix = 0.0f32;
            for v in voices.iter_mut() {
                mix += v.next_sample();
            }
            let pre = mix * MASTER_GAIN;
            let abs_pre = pre.abs();
            if abs_pre > self.peak_env {
                self.peak_env = abs_pre; // instant attack
            } else {
                self.peak_env *= self.limiter_release; // ~100 ms release
            }
            let gain = if self.peak_env > 1.0 {
                1.0 / self.peak_env
            } else {
                1.0
            };
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
        }
        voices.retain(|v| !v.finished());
    }
}

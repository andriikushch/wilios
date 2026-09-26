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

/// Every ADSR stage is floored to this many milliseconds so a `0` time (used
/// all over `lib/lib.wilios`, e.g. every drum's `attack 0`) becomes a fast
/// click-free fade (~2 ms ≈ inaudible) rather than a one-sample step.
const MIN_STAGE_MS: f32 = 2.0;
/// Attack/decay are one-pole "approach the target" curves that only reach the
/// target asymptotically; snap to it (and advance state) once this close.
const ATTACK_EPS: f32 = 1.0e-3;
/// Release decays multiplicatively toward this floor, then snaps to 0.
const REL_FLOOR: f32 = 1.0e-4;

/// One-pole coefficient `k` for `value += (target - value) * k` such that the
/// stage covers ~99% of the distance to the target in `ms` (≈ 5 time
/// constants). Used for attack and decay.
fn approach_coef(sample_rate: f32, ms: f32) -> f32 {
    let n = (ms.max(MIN_STAGE_MS) * sample_rate / 1000.0).max(1.0);
    let tau = n / 5.0;
    1.0 - (-1.0 / tau).exp()
}

/// Per-sample multiplier for the release stage: `value *= m` reaches
/// [`REL_FLOOR`] from `from` in exactly `ms`, so the note always finishes on
/// time regardless of the level it was released from.
fn release_multiplier(sample_rate: f32, ms: f32, from: f32) -> f32 {
    let n = (ms.max(MIN_STAGE_MS) * sample_rate / 1000.0).max(1.0);
    let v0 = from.max(REL_FLOOR);
    (REL_FLOOR / v0).powf(1.0 / n)
}

struct Envelope {
    value: f32,
    sustain_level: f32,
    attack_coef: f32,
    decay_coef: f32,
    release_mul: f32,
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
        Self {
            value: 0.0,
            sustain_level,
            attack_coef: approach_coef(sample_rate, attack_ms),
            decay_coef: approach_coef(sample_rate, decay_ms),
            release_mul: release_multiplier(sample_rate, release_ms, 1.0),
            state: EnvState::Attack,
        }
    }

    fn next(&mut self) -> f32 {
        match self.state {
            EnvState::Attack => {
                self.value += (1.0 - self.value) * self.attack_coef;
                if self.value >= 1.0 - ATTACK_EPS {
                    self.value = 1.0;
                    self.state = if self.sustain_level < 1.0 {
                        EnvState::Decay
                    } else {
                        EnvState::Sustain
                    };
                }
            }
            EnvState::Decay => {
                self.value += (self.sustain_level - self.value) * self.decay_coef;
                if self.value - self.sustain_level <= ATTACK_EPS {
                    self.value = self.sustain_level;
                    self.state = EnvState::Sustain;
                }
            }
            EnvState::Release => {
                self.value *= self.release_mul;
                if self.value <= REL_FLOOR {
                    self.value = 0.0;
                    self.state = EnvState::Off;
                }
            }
            _ => {}
        }
        self.value
    }

    fn note_off(&mut self, sample_rate: f32, release_ms: f32) {
        // Recompute the release curve from the current value so release always
        // completes within `release_ms`, monotonically, whatever level the
        // note was released from.
        self.release_mul = release_multiplier(sample_rate, release_ms, self.value);
        self.state = EnvState::Release;
    }

    fn finished(&self) -> bool {
        matches!(self.state, EnvState::Off)
    }
}

/// PolyBLEP (polynomial band-limited step) residual for a discontinuity at
/// phase wrap (0 / 1). `t` is the normalised phase in `[0, 1)`, `dt` the
/// per-sample phase increment in cycles. Subtract it from a naive sawtooth's
/// rising edge; add/subtract the two evaluations at a square's edges. This
/// rounds the one-sample jump over the `dt` on either side of the
/// discontinuity, cancelling most of the alias energy near Nyquist.
fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// Standalone waveform sampler — avoids borrowing the Voice struct.
///
/// `dt` is the oscillator's per-sample phase increment (cycles/sample), used to
/// band-limit `saw`/`square` via [`poly_blep`]. In the FM paths the phase
/// passed here is `base_phase + modulation`, so it is not a uniform ramp and
/// `dt` is only the nominal step — the correction is then approximate, but
/// still a clear improvement over the raw discontinuity for a `saw`/`square`
/// carrier. `sine`/`tri` do not use `dt`.
fn sample_waveform(wave: &Waveform, phase: f32, dt: f32) -> f32 {
    let t = phase.rem_euclid(1.0);
    match wave {
        Waveform::Sine => (t * 2.0 * PI).sin(),
        Waveform::Square => {
            let naive = if t < 0.5 { 1.0 } else { -1.0 };
            naive + poly_blep(t, dt) - poly_blep((t + 0.5).rem_euclid(1.0), dt)
        }
        Waveform::Saw => (2.0 * t - 1.0) - poly_blep(t, dt),
        Waveform::Triangle => {
            // Naive: harmonics fall off as 1/n^2, so aliasing is mild. A
            // band-limited triangle (leaky integrator of a BLEP square) adds an
            // f0-dependent level/DC term, so it is deferred.
            if t < 0.5 {
                t * 4.0 - 1.0
            } else {
                (1.0 - t) * 4.0 - 1.0
            }
        }
    }
}

/// TPT ("zero-delay-feedback") state-variable filter, low-pass tap. Coefficients
/// are fixed at note start; [`Svf::process`] runs one sample and returns the
/// low-pass output. This is the per-voice tone control behind the DSL's
/// `cutoff` / `resonance`.
struct Svf {
    a1: f32,
    a2: f32,
    a3: f32,
    ic1eq: f32,
    ic2eq: f32,
}

impl Svf {
    fn new(sample_rate: f32, cutoff_hz: f32, q: f32) -> Self {
        let fc = cutoff_hz.clamp(20.0, 0.45 * sample_rate);
        let g = (PI * fc / sample_rate).tan();
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        Self {
            a1,
            a2,
            a3,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    fn process(&mut self, v0: f32) -> f32 {
        let v3 = v0 - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        v2
    }
}

/// DSL `resonance` (0..1) → filter Q. 0 ≈ gently damped (Q 0.5); 1 ≈ Q 10,
/// just short of self-oscillation.
fn q_from_resonance(resonance: f32) -> f32 {
    0.5 + resonance.clamp(0.0, 1.0) * 9.5
}

/// Above this the low-pass is musically transparent, so the SVF isn't built and
/// the voice pays nothing for it. The DSL's default `cutoff` (20000) lands here.
const CUTOFF_OPEN_HZ: f32 = 18_000.0;

fn cutoff_is_open(cutoff_hz: f32, sample_rate: f32) -> bool {
    cutoff_hz >= CUTOFF_OPEN_HZ || cutoff_hz >= 0.45 * sample_rate
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
    /// Samples of silence before the note speaks, so an onset can land inside a
    /// buffer instead of on its boundary. Set by `delayed`; see
    /// `wilios_render::voices::drain_new_voices`.
    start_delay: u32,

    // Multi-op FM (Some = use multi-op path; None = use legacy path above)
    op_states: Option<Vec<OpState>>,
    // algorithm_indices[i] = (src_idx, dst_idx) as indices into op_states
    algorithm_indices: Vec<(usize, usize)>,
    // indices into op_states of operators that contribute to the audio output
    carrier_indices: Vec<usize>,
    // processing order: indices into op_states, modulators before their targets
    process_order: Vec<usize>,

    // Reusable per-sample scratch for the multi-op path, sized to op_states.len().
    // Overwritten every sample — kept on the Voice so next_sample() never allocates
    // (it runs on the realtime audio thread).
    scratch_outputs: Vec<f32>,
    scratch_env: Vec<f32>,

    // Tone shaping (DSL `cutoff` / `resonance`). None ⇒ cutoff is open.
    filter: Option<Svf>,
    // Vibrato LFO (DSL `vibrato <depth_cents> <rate_hz>`). Skipped when vib_k == 0.
    lfo_phase: f32,
    lfo_inc: f32,
    vib_k: f32,
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
        // Tone shaping — see the DSL `cutoff` / `resonance` / `vibrato` statements.
        // (A future cleanup could group these into a `ToneConfig` struct.)
        cutoff_hz: f32,
        resonance: f32,
        vibrato_depth_cents: f32,
        vibrato_rate_hz: f32,
    ) -> Self {
        let remaining_samples = (duration_ms as f32 / 1000.0 * sample_rate) as u64;

        let filter = if cutoff_is_open(cutoff_hz, sample_rate) {
            None
        } else {
            Some(Svf::new(
                sample_rate,
                cutoff_hz,
                q_from_resonance(resonance),
            ))
        };
        // cents → per-sample frequency multiplier `1 + vib_k * sin(lfo)`.
        let vib_k = (2.0_f32.ln() / 1200.0) * vibrato_depth_cents.max(0.0);
        let lfo_inc = if vib_k != 0.0 {
            vibrato_rate_hz.max(0.0) / sample_rate
        } else {
            0.0
        };

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
                start_delay: 0,
                sample_rate,
                op_states: Some(ops),
                algorithm_indices,
                carrier_indices,
                process_order,
                scratch_outputs: vec![0.0; n],
                scratch_env: vec![0.0; n],
                filter,
                lfo_phase: 0.0,
                lfo_inc,
                vib_k,
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
                start_delay: 0,
                sample_rate,
                op_states: None,
                algorithm_indices: Vec::new(),
                carrier_indices: Vec::new(),
                process_order: Vec::new(),
                scratch_outputs: Vec::new(),
                scratch_env: Vec::new(),
                filter,
                lfo_phase: 0.0,
                lfo_inc,
                vib_k,
            }
        }
    }

    /// Hold this voice silent for `samples` before it speaks.
    pub fn delayed(mut self, samples: u32) -> Self {
        self.start_delay = samples;
        self
    }

    pub fn next_sample(&mut self) -> f32 {
        // Sub-buffer onset: stay silent (and keep every phase, envelope and LFO
        // at its start) until the note is actually due.
        if self.start_delay > 0 {
            self.start_delay -= 1;
            return 0.0;
        }

        // Vibrato: one LFO evaluation per sample, applied as a frequency
        // multiplier to every oscillator so FM ratios are preserved. Skipped
        // entirely (and bit-identical to the old path) when depth is 0.
        let vib_mult = if self.vib_k != 0.0 {
            let m = 1.0 + self.vib_k * (self.lfo_phase * 2.0 * PI).sin();
            self.lfo_phase += self.lfo_inc;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
            m
        } else {
            1.0
        };

        let pre_filter = self.synth_sample(vib_mult);
        let shaped = match &mut self.filter {
            Some(f) => f.process(pre_filter),
            None => pre_filter,
        };
        shaped * self.volume
    }

    /// One sample of raw synthesis (carrier sum × envelopes), before the
    /// per-voice filter and `volume`. `vib_mult` scales every oscillator's
    /// phase increment this sample.
    fn synth_sample(&mut self, vib_mult: f32) -> f32 {
        if let Some(ops) = &mut self.op_states {
            // ---- Multi-operator FM synthesis ----
            let n = ops.len();

            // Reusable scratch — no allocation on the audio thread.
            // current_outputs starts from previous-sample outputs (feedback / initial values);
            // env_vals is cleared to 0.0 so an unresolved cycle member reads 0.0, exactly as a
            // freshly allocated buffer would.
            let current_outputs = &mut self.scratch_outputs;
            let env_vals = &mut self.scratch_env;
            for i in 0..n {
                current_outputs[i] = ops[i].last_output;
                env_vals[i] = 0.0;
            }

            for k in 0..self.process_order.len() {
                let idx = self.process_order[k];
                // Sum modulation from all ops that target this op.
                // Apply the source op's envelope so modulation depth tracks its ADSR over time.
                // Self-feedback (src == idx) uses 1.0 because env_vals[idx] isn't set yet.
                let mut modulation = 0.0f32;
                for &(src, dst) in &self.algorithm_indices {
                    if dst == idx {
                        let env = if src == idx { 1.0 } else { env_vals[src] };
                        modulation += current_outputs[src] * env * ops[src].level;
                    }
                }

                let raw = sample_waveform(
                    &ops[idx].wave,
                    ops[idx].phase + modulation,
                    ops[idx].phase_inc,
                );
                current_outputs[idx] = raw;
                env_vals[idx] = ops[idx].env.next();

                ops[idx].phase += ops[idx].phase_inc * vib_mult;
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
            let mut audio = 0.0f32;
            for &i in &self.carrier_indices {
                audio += current_outputs[i] * env_vals[i] * ops[i].level;
            }

            audio
        } else {
            // ---- Legacy 2-op FM synthesis ----
            let mod_signal = (self.mod_phase * 2.0 * PI).sin();
            let carrier_phase = self.phase + self.fm_depth * mod_signal;

            let s = sample_waveform(&self.waveform, carrier_phase, self.phase_inc);

            self.phase += self.phase_inc * vib_mult;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.mod_phase += self.mod_phase_inc * vib_mult;
            if self.mod_phase >= 1.0 {
                self.mod_phase -= 1.0;
            }

            if self.remaining_samples > 0 {
                self.remaining_samples -= 1;
                if self.remaining_samples == 0
                    && matches!(
                        self.env.state,
                        EnvState::Attack | EnvState::Sustain | EnvState::Decay
                    )
                {
                    // Includes Attack so a note shorter than its attack still
                    // releases and the voice eventually reports finished().
                    self.env.note_off(self.sample_rate, self.release_ms);
                }
            }

            s * self.env.next()
        }
    }

    pub fn finished(&self) -> bool {
        if self.start_delay > 0 {
            return false; // not started yet
        }
        if let Some(ops) = &self.op_states {
            self.remaining_samples == 0 && ops.iter().all(|o| o.env.finished())
        } else {
            self.remaining_samples == 0 && self.env.finished()
        }
    }
}

const MASTER_GAIN: f32 = 0.3;

/// One-pole DC blocker: `y[n] = x[n] - x[n-1] + r * y[n-1]`, `r` just below 1
/// so the high-pass corner sits a few Hz above 0. Removes the standing offset
/// that asymmetric FM and the soft-clip leave behind.
fn dc_block(r: f32, x: f32, x1: &mut f32, y1: &mut f32) -> f32 {
    let y = x - *x1 + r * *y1;
    *x1 = x;
    *y1 = y;
    y
}

/// Mixes active voices into an output buffer, applying a smoothed peak limiter,
/// a safety soft-clip, and a DC blocker. Owns the limiter's running peak
/// envelope and smoothed gain so state persists correctly across successive
/// audio-callback buffers.
pub struct Mixer {
    channels: usize,
    /// One-pole rise coefficient for the peak follower (~3 ms) — no longer an
    /// instantaneous jump, so a single loud sample can't slam the whole mix.
    limiter_attack: f32,
    /// One-pole fall coefficient for the peak follower (~120 ms).
    limiter_release: f32,
    peak_env: f32,
    /// Smoothed gain actually applied — slewed toward `1/peak_env` so it never
    /// steps hard (bounded |Δgain| per sample), which is what removed the
    /// pumping.
    gain: f32,
    gain_slew: f32,
    dc_r: f32,
    dc_x1: f32,
    dc_y1: f32,
}

impl Mixer {
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            channels,
            limiter_attack: 1.0 - (-1.0_f32 / (sr * 0.003)).exp(),
            limiter_release: (-1.0_f32 / (sr * 0.12)).exp(),
            peak_env: 0.0,
            gain: 1.0,
            gain_slew: 1.0 - (-1.0_f32 / (sr * 0.005)).exp(),
            dc_r: 1.0 - (2.0 * PI * 10.0 / sr),
            dc_x1: 0.0,
            dc_y1: 0.0,
        }
    }

    /// Advances `voices` by one buffer's worth of samples, writing the mixed,
    /// limited, saturated, DC-blocked output into `data` (interleaved by
    /// `channels`), and drops any voices that finished during this buffer.
    pub fn process(&mut self, voices: &mut Vec<Voice>, data: &mut [f32]) {
        for frame in data.chunks_mut(self.channels) {
            let mut mix = 0.0f32;
            for v in voices.iter_mut() {
                mix += v.next_sample();
            }
            let pre = mix * MASTER_GAIN;

            // Smoothed peak follower: quick (not instant) rise, slow fall.
            let target = pre.abs();
            if target > self.peak_env {
                self.peak_env += (target - self.peak_env) * self.limiter_attack;
            } else {
                self.peak_env *= self.limiter_release;
            }
            let want = if self.peak_env > 1.0 {
                1.0 / self.peak_env
            } else {
                1.0
            };
            // Slew the applied gain so it can't step — this is what stops the
            // limiter from breathing on every transient.
            self.gain += (want - self.gain) * self.gain_slew;
            let x = pre * self.gain;

            // Safety soft-clip: linear below the knee, tanh rolloff above.
            const KNEE: f32 = 0.7_f32;
            let abs_x = x.abs();
            let clipped = if abs_x < KNEE {
                x
            } else {
                let headroom = 1.0_f32 - KNEE;
                let excess = (abs_x - KNEE) / headroom;
                x.signum() * (KNEE + headroom * excess.tanh())
            };

            let out = dc_block(self.dc_r, clipped, &mut self.dc_x1, &mut self.dc_y1);
            for ch in frame.iter_mut() {
                *ch = out;
            }
        }
        voices.retain(|v| !v.finished());
    }

    #[cfg(test)]
    fn applied_gain(&self) -> f32 {
        self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// `Voice::new` with tone shaping (cutoff/resonance/vibrato) at their
    /// neutral defaults — an open filter and no vibrato.
    #[allow(clippy::too_many_arguments)]
    fn plain_voice(
        freq: f32,
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
    ) -> Voice {
        Voice::new(
            freq,
            SR,
            volume,
            duration_ms,
            waveform,
            attack_ms,
            decay_ms,
            sustain_level,
            release_ms,
            fm_ratio,
            fm_depth,
            fm_block,
            20_000.0,
            0.0,
            0.0,
            0.0,
        )
    }

    // ---- PolyBLEP oscillators -------------------------------------------------

    /// Ideal band-limited rising saw (±1), first `k` harmonics, at `phase`
    /// cycles: `2t-1 = -(2/π) Σ_{n≥1} sin(2π n t)/n`.
    fn saw_reference(phase: f32, k: usize) -> f32 {
        let mut acc = 0.0f32;
        for n in 1..=k {
            acc += (2.0 * PI * n as f32 * phase).sin() / n as f32;
        }
        -(2.0 / PI) * acc
    }

    #[test]
    fn polyblep_saw_reduces_aliasing() {
        // 6 kHz saw at 44.1 kHz: harmonics 6/12/18 kHz are below Nyquist,
        // 24 kHz+ fold back as inharmonic alias tones.
        let sr = 44_100.0f32;
        let f0 = 6_000.0f32;
        let dt = f0 / sr;
        let k = (sr / 2.0 / f0).floor() as usize; // 3

        let mut phase = 0.0f32;
        let mut naive_err = 0.0f64;
        let mut blep_err = 0.0f64;
        for i in 0..4096 {
            let t = phase.rem_euclid(1.0);
            let reference = saw_reference(t, k);
            let naive = 2.0 * t - 1.0;
            let blep = sample_waveform(&Waveform::Saw, phase, dt);
            if i >= 128 {
                naive_err += ((naive - reference) as f64).powi(2);
                blep_err += ((blep - reference) as f64).powi(2);
            }
            phase = (phase + dt).rem_euclid(1.0);
        }
        assert!(
            blep_err < 0.6 * naive_err,
            "PolyBLEP saw error {blep_err:.3} not well below naive {naive_err:.3}"
        );
    }

    #[test]
    fn sine_and_tri_unaffected_by_dt() {
        for w in [Waveform::Sine, Waveform::Triangle] {
            for i in 0..64 {
                let p = i as f32 / 64.0;
                assert_eq!(
                    sample_waveform(&w, p, 0.0),
                    sample_waveform(&w, p, 0.01),
                    "waveform {w:?} should ignore dt at phase {p}"
                );
            }
        }
    }

    // ---- Exponential ADSR ---------------------------------------------------

    #[test]
    fn envelope_attack_zero_no_click() {
        // attack/decay/release all 0 -> floored to MIN_STAGE_MS, so no
        // single-sample jump (the old code stepped a full 1.0).
        let mut env = Envelope::new(SR, 0.0, 0.0, 0.7, 0.0);
        let mut prev = 0.0f32;
        let mut max_step = 0.0f32;
        for _ in 0..512 {
            let v = env.next();
            max_step = max_step.max((v - prev).abs());
            prev = v;
        }
        assert!(max_step < 0.08, "max envelope step {max_step} — clicks");
    }

    #[test]
    fn envelope_release_completes_within_release_ms() {
        let mut env = Envelope::new(SR, 5.0, 0.0, 1.0, 50.0);
        for _ in 0..1_000 {
            env.next(); // settle into Sustain
        }
        env.note_off(SR, 50.0);
        let budget = (50.0 * SR / 1000.0).ceil() as usize + 8;
        let mut done_at = None;
        for i in 0..budget {
            env.next();
            if env.finished() {
                done_at = Some(i);
                break;
            }
        }
        assert!(done_at.is_some(), "release did not finish within {budget}");
    }

    #[test]
    fn envelope_short_note_still_releases() {
        // Legacy-path bug: a note shorter than its attack never released.
        // Envelope itself must terminate once note_off is called mid-attack.
        let mut env = Envelope::new(SR, 500.0, 0.0, 1.0, 20.0);
        for _ in 0..10 {
            env.next();
        }
        env.note_off(SR, 20.0);
        let mut finished = false;
        for _ in 0..2_000 {
            env.next();
            if env.finished() {
                finished = true;
                break;
            }
        }
        assert!(finished, "envelope stuck after mid-attack note_off");
    }

    #[test]
    fn envelope_is_monotone_per_segment() {
        let mut env = Envelope::new(SR, 10.0, 80.0, 0.4, 60.0);
        let mut vals = Vec::new();
        for _ in 0..3_000 {
            vals.push(env.next());
        }
        env.note_off(SR, 60.0);
        for _ in 0..6_000 {
            vals.push(env.next());
            if env.finished() {
                break;
            }
        }
        let peak_idx = vals
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        for w in vals[..=peak_idx].windows(2) {
            assert!(w[1] >= w[0] - 1e-6, "attack not monotone up");
        }
        for w in vals[peak_idx..].windows(2) {
            assert!(w[1] <= w[0] + 1e-6, "decay/release not monotone down");
        }
    }

    // ---- DC blocker ------------------------------------------------------------

    #[test]
    fn dc_blocker_removes_constant_offset() {
        let r = 1.0 - (2.0 * PI * 10.0 / SR);
        let (mut x1, mut y1) = (0.0f32, 0.0f32);
        let mut last = Vec::new();
        for i in 0..8_192 {
            let y = dc_block(r, 0.5, &mut x1, &mut y1);
            if i >= 7_168 {
                last.push(y);
            }
        }
        let mean = last.iter().sum::<f32>() / last.len() as f32;
        assert!(mean.abs() < 1e-3, "DC not removed, residual mean {mean}");
    }

    #[test]
    fn dc_blocker_passes_audio_band() {
        let r = 1.0 - (2.0 * PI * 10.0 / SR);
        let (mut x1, mut y1) = (0.0f32, 0.0f32);
        let mut in_sq = 0.0f64;
        let mut out_sq = 0.0f64;
        for i in 0..8_192 {
            let x = (2.0 * PI * 220.0 * i as f32 / SR).sin();
            let y = dc_block(r, x, &mut x1, &mut y1);
            if i >= 2_048 {
                in_sq += (x as f64).powi(2);
                out_sq += (y as f64).powi(2);
            }
        }
        let ratio = (out_sq / in_sq).sqrt();
        assert!(ratio > 0.97, "220 Hz attenuated too much: {ratio}");
    }

    // ---- Smoothed limiter ---------------------------------------------------

    #[test]
    fn limiter_gain_continuous() {
        let mut mixer = Mixer::new(SR, 1);
        // One deliberately hot voice: 6.0 * MASTER_GAIN(0.3) = 1.8 peak.
        let mut voices = vec![plain_voice(
            220.0,
            6.0,
            2_000,
            Waveform::Sine,
            5.0,
            0.0,
            1.0,
            200.0,
            1.0,
            0.0,
            None,
        )];
        let mut prev = mixer.applied_gain();
        let mut buf = [0.0f32; 1];
        let mut max_step = 0.0f32;
        for _ in 0..8_000 {
            mixer.process(&mut voices, &mut buf);
            let g = mixer.applied_gain();
            max_step = max_step.max((g - prev).abs());
            prev = g;
        }
        assert!(
            prev < 1.0,
            "limiter never engaged on a hot mix (gain {prev})"
        );
        assert!(
            max_step < 0.01,
            "limiter gain stepped by {max_step} — will pump"
        );
    }

    // ---- Filter + vibrato -------------------------------------------------

    fn rms_through_svf(freq: f32, cutoff: f32, q: f32) -> f32 {
        let mut svf = Svf::new(SR, cutoff, q);
        let mut in_sq = 0.0f64;
        let mut out_sq = 0.0f64;
        for i in 0..16_384 {
            let x = (2.0 * PI * freq * i as f32 / SR).sin();
            let y = svf.process(x);
            if i >= 4_096 {
                in_sq += (x as f64).powi(2);
                out_sq += (y as f64).powi(2);
            }
        }
        (out_sq / in_sq).sqrt() as f32
    }

    #[test]
    fn svf_lowpass_rolloff() {
        // Well below cutoff: passes ~unchanged.
        let pass = rms_through_svf(250.0, 1_000.0, 0.707);
        assert!(
            pass > 0.9,
            "250 Hz through a 1 kHz LP lost too much: {pass}"
        );
        // One octave above cutoff: ~-12 dB for a 2nd-order LP (ratio ~0.25).
        let stop = rms_through_svf(2_000.0, 1_000.0, 0.707);
        assert!(
            (0.15..0.40).contains(&stop),
            "2 kHz through a 1 kHz LP: {stop}, expected ~0.25"
        );
    }

    #[test]
    fn filter_open_is_bypassed() {
        // A high cutoff means no SVF is built; a nonzero resonance alongside it
        // must not matter.
        let mut a = plain_voice(
            440.0,
            1.0,
            500,
            Waveform::Saw,
            5.0,
            0.0,
            1.0,
            100.0,
            1.0,
            0.0,
            None,
        );
        let mut b = Voice::new(
            440.0,
            SR,
            1.0,
            500,
            Waveform::Saw,
            5.0,
            0.0,
            1.0,
            100.0,
            1.0,
            0.0,
            None,
            30_000.0,
            0.6,
            0.0,
            0.0,
        );
        assert!(a.filter.is_none() && b.filter.is_none());
        for _ in 0..2_000 {
            assert_eq!(a.next_sample(), b.next_sample());
        }
    }

    #[test]
    fn vibrato_zero_depth_is_identity() {
        let mk = |depth: f32, rate: f32| {
            Voice::new(
                330.0,
                SR,
                1.0,
                600,
                Waveform::Saw,
                5.0,
                0.0,
                1.0,
                100.0,
                1.0,
                0.0,
                None,
                20_000.0,
                0.0,
                depth,
                rate,
            )
        };
        let mut a = mk(0.0, 0.0);
        let mut b = mk(0.0, 6.0); // rate set, but depth 0 ⇒ no modulation
        for _ in 0..4_000 {
            assert_eq!(a.next_sample(), b.next_sample());
        }
    }

    #[test]
    fn voice_cutoff_and_vibrato_smoke() {
        let mut v = Voice::new(
            440.0,
            SR,
            1.0,
            300,
            Waveform::Sine,
            20.0,
            100.0,
            0.7,
            150.0,
            1.0,
            0.0,
            None,
            2_000.0,
            0.3,
            25.0,
            5.5,
        );
        assert!(v.filter.is_some());
        for _ in 0..SR as usize {
            let s = v.next_sample();
            assert!(s.is_finite() && s.abs() < 4.0, "runaway sample {s}");
        }
    }
}

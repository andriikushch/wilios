# wilios Synthesis Reference

This document covers the sound synthesis system: waveforms, ADSR envelopes, performance controls, and FM synthesis (both the legacy 2-op mode and the multi-operator FM block).

---

## Table of Contents

1. [Default Parameter Values](#1-default-parameter-values)
2. [Performance Controls](#2-performance-controls)
   - [Tempo](#tempo)
   - [Volume](#volume)
   - [Pan](#pan)
   - [Swing](#swing)
   - [Time Signature](#time-signature)
3. [Waveforms](#3-waveforms)
4. [ADSR Envelope](#4-adsr-envelope)
5. [Legacy 2-Op FM Synthesis](#5-legacy-2-op-fm-synthesis)
6. [Multi-Operator FM Block](#6-multi-operator-fm-block)
   - [Algorithm (Routing)](#61-algorithm-routing)
   - [Operator Declaration](#62-operator-declaration)
   - [Self-Feedback](#63-self-feedback)
   - [Processing Order](#64-processing-order)
   - [Defaults and Inheritance](#65-defaults-and-inheritance)
7. [Synthesis Pipeline](#7-synthesis-pipeline)
8. [FM Synthesis Concepts](#8-fm-synthesis-concepts)

---

## 1. Default Parameter Values

All synthesis parameters are **per-track**. When a track starts it inherits these hardcoded defaults, unless a `global`-scope statement set a different default first (see [language-reference.md — Scope](language-reference.md#11-scope)) — either way, a track can still override any of them locally:

| Parameter | Default | Range / Unit |
|-----------|---------|--------------|
| `tempo` | `120` | BPM, integer |
| `volume` | `100` | 0–127, integer |
| `pan` | `0` | −127 (left) to +127 (right) |
| `time_signature` | `4/4` | numerator/denominator, both > 0 (metadata only) |
| `wave` | `sine` | `sine` `square` `saw` `tri` |
| `attack` | `10` | milliseconds, ≥ 0 |
| `decay` | `0` | milliseconds, ≥ 0 |
| `sustain` | `100` | level 0–100 (percentage) |
| `release` | `100` | milliseconds, ≥ 0 |
| `fm_ratio` | `1.0` | ratio (modulator freq / carrier freq) |
| `fm_depth` | `0.0` | modulation index; 0 = FM disabled |
| `fm_block` | none | multi-op FM block (overrides legacy FM) |
| `swing` | `50` | swing feel: 50 = straight, 100 = maximum swing |

---

## 2. Performance Controls

### Tempo

Sets the playback speed in **beats per minute**. All note and rest durations on this track are scaled accordingly. Accepts a literal integer.

```
tempo integer
```

```wilios
tempo 120    // standard tempo
tempo 80     // slow
tempo 180    // fast
```

Changing tempo mid-track takes effect from the next note:

```wilios
tempo 120
<C4> 1/4    // at 120 BPM
tempo 60
<C4> 1/4    // at 60 BPM (twice as slow)
```

### Volume

Sets the output amplitude for this track. Range `0` (silent) to `127` (maximum). Accepts a literal integer.

```
volume integer
```

```wilios
volume 100   // default
volume 40    // quiet
volume 127   // loud
```

### Pan

Sets the stereo position. `0` is centre, `−127` is full left, `+127` is full right. Accepts a literal integer (optionally prefixed with `−`).

```
pan integer
pan -integer
```

```wilios
pan 0        // centre
pan -64      // left of centre
pan 64       // right of centre
pan -127     // hard left
pan 127      // hard right
```

### Swing

Applies a swing feel to 8th-note pairs. Notes on the **on-beat** (even) 8th-note slot are lengthened; notes on the **off-beat** (odd) slot are shortened. Quarter notes and longer values are unaffected. Notes shorter than an 8th pass through unchanged.

```
swing integer_or_float
```

| Value | Effect |
|-------|--------|
| `50` | Straight — equal 8th notes (default) |
| `67` | Classic jazz/blues swing (~⅔ + ⅓ feel) |
| `75` | Heavy swing |
| `100` | Maximum swing (on-beat takes the full quarter, off-beat = 0ms) |

Valid range: **50–100** (inclusive). Values outside this range are a runtime error.

```wilios
tempo 120
swing 67         // 8th notes: 335ms + 165ms (= 500ms quarter)

<C4> 1/8         // on-beat → 335ms (long)
<D4> 1/8         // off-beat → 165ms (short)
<E4> 1/4         // quarter note → always 500ms, unaffected
```

```wilios
// Back to straight
swing 50
<C4> 1/8         // 250ms
<D4> 1/8         // 250ms
```

Rests also consume time with swing applied, so the phase is preserved:

```wilios
swing 67
rest 1/8         // on-beat rest → 335ms; next note lands on off-beat
<C4> 1/8         // off-beat → 165ms (short)
```

Both integer and float literals are accepted: `swing 67` and `swing 67.0` are equivalent.

### Time Signature

Declares the meter as `numerator/denominator`. This is **metadata only**: it's stamped onto every emitted Note event, but wilios has no bar/measure concept, so it has no effect on duration or timing math.

```
time_signature integer/integer
```

```wilios
time_signature 4/4   // default
time_signature 3/4   // waltz time
time_signature 6/8   // compound time
```

Both numerator and denominator must be greater than 0; violating this is a runtime error. Settable in global scope (a default for every track) or per track (a local override):

```wilios
time_signature 3/4    // default for every track

track 1
<C4> 1/4               // inherits 3/4 (not shown on this event's audio — metadata only)

track 2
time_signature 6/8     // overrides the global default here
<G3> 1/4
```

---

## 3. Waveforms

The `wave` statement selects the oscillator waveform for the **carrier** operator (or all notes if not using FM).

```
wave waveform
```

| Waveform | Keyword | Character |
|----------|---------|-----------|
| Sine | `sine` | Smooth, pure tone with no overtones |
| Square | `square` | Bright, hollow; strong odd harmonics |
| Sawtooth | `saw` | Bright, buzzy; rich harmonics |
| Triangle | `tri` | Soft, flute-like; weak odd harmonics |

```wilios
wave sine     // default — pure, smooth
wave square   // bright and hollow
wave saw      // rich, buzzy
wave tri      // soft, rounded
```

The waveform applies to all notes played after the statement until changed. Inside an FM block, individual operators can also specify their own waveform (see [Section 6.2](#62-operator-declaration)).

---

## 4. ADSR Envelope

The ADSR envelope shapes the amplitude of each note over time. All four stages are specified in **milliseconds** (except `sustain` which is a level 0–100).

```
attack  expr    // rise time from 0 to peak (ms)
decay   expr    // fall time from peak to sustain level (ms)
sustain expr    // level held while note is on (0 = silent, 100 = full)
release expr    // fall time from sustain level to 0 after note ends (ms)
```

```
Amplitude
  │     /\
  │    /  \
  │   /    \______
  │  /             \
  │ /               \
  └──────────────────── time
    A   D    S    R
```

### Stage Descriptions

| Stage | Parameter | Description |
|-------|-----------|-------------|
| Attack | `attack` | How long the note takes to reach full amplitude from silence |
| Decay | `decay` | How long it takes to fall from peak to the sustain level |
| Sustain | `sustain` | The steady-state amplitude level held until the note ends |
| Release | `release` | How long the note takes to fade to silence after it ends |

### Examples

**Plucked string** — fast attack, medium decay, no sustain, short release:

```wilios
attack 5
decay 400
sustain 0
release 80
```

**Organ** — instant attack, no decay, full sustain, instant release:

```wilios
attack 0
decay 0
sustain 100
release 20
```

**Pad / strings** — slow attack and release, full sustain:

```wilios
attack 500
decay 0
sustain 100
release 800
```

**Percussive** — instant attack, fast decay, no sustain:

```wilios
attack 0
decay 200
sustain 0
release 50
```

### ADSR Values as Expressions

The `attack`, `decay`, `sustain`, and `release` statements accept full expressions (variables, arithmetic):

```wilios
let slow = 600
attack slow
release slow * 2
```

---

## 5. Legacy 2-Op FM Synthesis

The legacy FM mode provides simple two-operator frequency modulation without declaring an explicit `fm { }` block.

```
fm_ratio numeric    // modulator freq = carrier freq × ratio
fm_depth numeric    // modulation index (0.0 = no FM)
```

Both statements accept only **literal numeric values** (integers or floats).

### How it works

```
Modulator oscillator:  freq = carrier_freq × fm_ratio
                       phase += fm_depth × sin(mod_phase)
                                ↓
                       modulates carrier phase
                                ↓
Carrier oscillator:    produces the audible output
```

When `fm_depth` is `0.0` (the default) the modulator has no effect and the track produces pure waveform synthesis. Setting `fm_depth > 0` introduces harmonic content whose character depends on `fm_ratio`.

### Parameter Effects

| `fm_ratio` | Character |
|-----------|-----------|
| `1.0` | Unison modulation — adds richness, stays harmonic |
| `2.0` | Octave modulation — bright, harmonic overtones |
| `0.5` | Sub-octave modulation — thick, warm, bass-like |
| Inharmonic (e.g. `3.5`) | Metallic, bell-like, inharmonic |

| `fm_depth` | Character |
|-----------|-----------|
| `0.0` | No FM (pure waveform) |
| `0.1–1.0` | Subtle harmonic coloration |
| `1.0–3.0` | Rich, complex timbre |
| `3.0+` | Very bright or noisy |

### Example

```wilios
wave sine
fm_ratio 2.0
fm_depth 1.5
attack 10
decay 300
sustain 0
release 100

<A4> 1/4
```

> **Note:** When an `fm { }` block is set on a track, it completely **replaces** the legacy FM parameters for that track. The legacy `fm_ratio`/`fm_depth` settings are ignored while an FM block is active.

---

## 6. Multi-Operator FM Block

The `fm { }` block configures N-operator FM synthesis with arbitrary routing. It provides far more expressive control than the legacy 2-op mode.

```
fm {
    algorithm [routing_edge, ...]
    op integer { op_fields }
    op integer { op_fields }
    ...
}
```

Setting an FM block on a track **replaces** any previous FM block and disables the legacy `fm_ratio`/`fm_depth` settings.

### 6.1 Algorithm (Routing)

The `algorithm` declaration defines which operators modulate which others, using arrow notation:

```
algorithm [src -> dst, ...]
```

Each edge `src -> dst` means: operator `src`'s output is used to **phase-modulate** operator `dst`.

An operator with no outgoing edges (it modulates no one) is a **carrier** — its output is summed into the final audio output.

An operator with no incoming edges (no one modulates it) is a pure **modulator**.

> **Self-loops:** A self-edge (`N->N`) makes operator `N` both a source and a destination, so on its own it satisfies neither definition above. The carrier set is "every operator that is never a source"; if `algorithm [1->1]` is the *only* edge, that set is empty, so the engine falls back to treating the first-declared operator as the carrier — this fallback fires whenever routing leaves no operator without an outgoing edge, not just for this specific case. See [§6.3 — Self-Feedback](#63-self-feedback).

```wilios
// Simple 2-op: op 2 modulates op 1 (op 1 is the carrier)
algorithm [2->1]

// Three operators in a chain: 3 → 2 → 1
algorithm [3->2, 2->1]

// Two modulators on one carrier
algorithm [2->1, 3->1]

// Two independent 2-op pairs (algorithm 5 style)
algorithm [2->1, 4->3]
// op 1 and op 3 are both carriers; their outputs are summed
```

#### Common Algorithms

**Single carrier + modulator (2-op basic FM):**

```
algorithm [2->1]

Op 2 (mod) ──modulates──> Op 1 (carrier) ──> output
```

**Stack (3-op chain):**

```
algorithm [3->2, 2->1]

Op 3 ──> Op 2 ──> Op 1 ──> output
```

**Two parallel modulators:**

```
algorithm [2->1, 3->1]

Op 2 ──\
        ──modulate──> Op 1 ──> output
Op 3 ──/
```

**Two independent pairs:**

```
algorithm [2->1, 4->3]

Op 2 ──> Op 1 ──> \
                   ──summed──> output
Op 4 ──> Op 3 ──> /
```

### 6.2 Operator Declaration

Each operator is declared with `op N { ... }`:

```
op integer {
    ratio   expr
    level   expr
    wave    waveform
    attack  expr
    decay   expr
    sustain expr
    release expr
}
```

All fields are optional. Missing fields inherit defaults (see [Section 6.5](#65-defaults-and-inheritance)).

| Field | Description | Default |
|-------|-------------|---------|
| `ratio` | Operator frequency = note frequency × ratio | `1.0` |
| `level` | Output amplitude (modulation depth for modulators, mix level for carriers) | `1.0` |
| `wave` | Oscillator waveform for this operator | track's `wave` setting |
| `attack` | Operator envelope attack (ms) | track's `attack` setting |
| `decay` | Operator envelope decay (ms) | track's `decay` setting |
| `sustain` | Operator envelope sustain level (0–100) | track's `sustain` setting |
| `release` | Operator envelope release (ms) | track's `release` setting |

#### Ratio and Level

For a **modulator** (operator that modulates another):
- `ratio` controls the frequency of the modulating oscillator relative to the note frequency
- `level` controls the **modulation depth** (modulation index) — higher values produce richer, louder harmonics

For a **carrier** (operator whose output goes to audio):
- `ratio` controls the frequency of the output oscillator (usually `1.0` for the fundamental)
- `level` controls the amplitude contribution to the final mix

#### Example — Full Operator Specification

```wilios
fm {
    algorithm [2->1]
    op 1 { ratio 1.0  level 1.0  wave sine  attack 10  decay 0    sustain 100  release 200 }
    op 2 { ratio 2.0  level 3.0  wave sine  attack 0   decay 400  sustain 0    release 100 }
}
```

#### Minimal FM Block

Fields not listed inherit from the track defaults:

```wilios
fm {
    algorithm [2->1]
    op 1 { level 1.0 }
    op 2 { ratio 2.0  level 2.0  decay 300  sustain 0 }
}
```

### 6.3 Self-Feedback

An operator can modulate itself. This is written as `N->N` in the algorithm:

```
algorithm [1->1]
```

Because a self-edge gives that operator both an incoming and an outgoing edge, a bare `algorithm [1->1]` doesn't cleanly fit either definition from [§6.1](#61-algorithm-routing) — the engine's fallback rule picks it as the carrier anyway (see the note there). In practice, self-feedback is normally combined with other routing, as in the example below, where the carrier is unambiguous.

Self-feedback introduces a one-sample delay in the feedback path. Low feedback levels add warmth; high levels add noise and aliasing artefacts. Self-feedback is most commonly applied to a single modulator operator.

```wilios
fm {
    algorithm [2->1, 2->2]   // op 2 self-feeds, also modulates op 1
    op 1 { ratio 1.0  level 1.0 }
    op 2 { ratio 1.0  level 1.5 }
}
```

### 6.4 Processing Order

Operators are processed in **topological order** (Kahn's algorithm):

1. Find all operators with no incoming modulation edges (leaf modulators).
2. Process them first — their outputs are available for operators that depend on them.
3. Continue up the dependency graph until carriers are reached.
4. Self-feedback edges use the **previous sample's** output (one-sample delay).

This means the routing can support:
- Chains of arbitrary depth (e.g. 6-op chains)
- Multiple modulators per carrier
- Parallel carrier paths (summed)
- Self-feedback on any operator

### 6.5 Defaults and Inheritance

When an FM block is active:

- **Operator `ratio`**: defaults to `1.0` if not specified
- **Operator `level`**: defaults to `1.0` if not specified
- **Operator `wave`**: defaults to the track's current `wave` setting
- **Operator ADSR**: each field independently inherits from the track's corresponding ADSR setting if not specified

This means you can set track-wide ADSR and only override per-operator values where needed:

```wilios
// Set track-wide fast decay
attack 5
decay 200
sustain 0
release 80

fm {
    algorithm [2->1]
    op 1 { ratio 1.0  level 1.0  release 300 }  // override release only
    op 2 { ratio 3.5  level 2.0 }               // inherits all ADSR from track
}
```

---

## 7. Synthesis Pipeline

For each note event, the audio engine instantiates a **Voice**. Two synthesis paths exist:

### Path A — Legacy 2-Op (no FM block)

```
note_freq = note_to_hz(pitch)

modulator:   mod_freq   = note_freq × fm_ratio
             mod_phase += mod_freq  / sample_rate
             mod_out    = fm_depth × sin(mod_phase)

carrier:     car_phase += (note_freq / sample_rate) + mod_out
             sample     = waveform(car_phase) × ADSR_envelope × volume
```

When `fm_depth == 0`, the modulator contributes nothing and the carrier produces a pure waveform.

### Path B — Multi-Operator FM Block

```
For each sample:
  1. Process operators in topological order
  2. For each operator i:
       phase_i    += (note_freq × ratio_i / sample_rate) + sum(phase_mod from modulators of i)
       env_i       = evaluate ADSR envelope for operator i
       output_i    = waveform_i(phase_i) × level_i × env_i
  3. Sum outputs of all carrier operators (operators with no outgoing edges)
  4. Apply track volume and pan
  5. Mix into audio buffer
```

Self-feedback uses the previous sample's output of that operator, stored in a per-voice state variable.

### Soft-Clip Limiter

The final mixed output passes through a `tanh`-based soft-clip limiter to prevent hard clipping when multiple tracks are mixed.

---

## 8. FM Synthesis Concepts

### What is FM Synthesis?

Frequency Modulation (FM) synthesis generates sound by modulating the **frequency** (or phase) of one oscillator (the carrier) with the output of another oscillator (the modulator). This interaction produces sidebands — new frequencies above and below the carrier — creating complex timbres from simple oscillators.

### Key Parameters

**Carrier frequency** — The fundamental pitch of the note being played.

**Modulator ratio** — Multiplier applied to the carrier frequency to get the modulator's frequency. Integer ratios (1, 2, 3…) produce harmonic sidebands; non-integer ratios (1.5, 3.5, 7.07…) produce inharmonic (metallic, bell-like) sidebands.

**Modulation index / level** — How strongly the modulator affects the carrier. Low index = subtle harmonic coloration. High index = very bright or noisy timbre.

**Operator ADSR** — Each operator has its own envelope. A modulator whose level decays quickly produces a bright attack that settles into a pure tone — the hallmark of electric piano, marimba, and similar sounds.

### Timbre Design Tips

| Goal | Approach |
|------|----------|
| Pure tone | `fm_depth 0` or single carrier with no modulators |
| Bell / marimba | Inharmonic modulator ratios (3.5, 5.0); fast decay, no sustain |
| Electric piano | High-ratio modulator (14×); both carrier and modulator fully percussive |
| Brass | Same-ratio modulator (1.0) at high level, medium decay on modulator |
| Bass | Sub-octave modulator (0.5); fast modulator decay |
| Strings / pads | Slow attack on both carrier and modulator; gentle modulation level |
| Drums | Very high modulator level + fast decay to create pitch drop/transient |
| Noise-like | Prime-ratio modulators (11, 17, 17.5…); square carrier |

### Algorithm Selection

- **2-op (single pair):** Use for basic FM timbres — bass, brass, simple bells.
- **3-op chain:** Use for more complex spectra where the modulator itself is modulated (adds sidebands of sidebands).
- **2 pairs (algo 5 style):** Use for layered timbres — the two carriers are summed, each with its own character. Good for electric piano (two bell pairs), strings (fundamental + octave).
- **Multiple modulators on one carrier:** Use when multiple independent tonal components should shape the same fundamental.

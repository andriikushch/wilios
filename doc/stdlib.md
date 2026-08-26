# wilios Standard Library

The standard library is a single importable file located at `lib/lib.wilios`. It provides:

- **9 FM synthesis instrument presets** — ready-made `fm { }` blocks for common instrument categories
- **4 built-in functions** — native functions available in every program without importing anything

---

## Table of Contents

1. [Built-in Functions](#1-built-in-functions)
   - [print](#print)
   - [rand](#randmin-max)
   - [transpose](#transposepitch_or_chord-semitones)
   - [len](#lenarray)
2. [Importing the Standard Library](#2-importing-the-standard-library)
3. [Preset Overview](#3-preset-overview)
4. [Tonal Presets](#4-tonal-presets)
   - [epiano](#41-epiano)
   - [brass](#42-brass)
   - [bass](#43-bass)
   - [marimba](#44-marimba)
   - [strings](#45-strings)
5. [Drum Presets](#5-drum-presets)
   - [kick](#51-kick)
   - [snare](#52-snare)
   - [hihat\_c](#53-hihat_c)
   - [hihat\_o](#54-hihat_o)
6. [Building Custom Presets](#6-building-custom-presets)

---

## 1. Built-in Functions

These functions are available in **every** wilios program. They do not require an import.

---

### `print(...)`

Print one or more values to standard output, separated by spaces.

**Signature:** `print(value, value, ...) -> Int`
**Returns:** `0` (Int)

```wilios
print("hello")            // hello
print(42)                 // 42
print(true)               // true
print(C4)                 // C4
print(<C4, E4, G4>)       // <C4, E4, G4>
print(1, 2, 3)            // 1 2 3
```

Value formatting:

| Type | Format |
|------|--------|
| `Int` | Decimal integer, e.g. `42` |
| `Float` | String representation, e.g. `3.14` |
| `Bool` | `true` or `false` |
| `Pitch` | Letter + accidental + octave, e.g. `C#4` |
| `Chord` | `<p1, p2, ...>`, e.g. `<C4, E4, G4>` |
| `Array` | `[elem1, elem2, ...]`, e.g. `[1, 2, 3]` |
| `Func` | `<func>` |
| `Builtin` | `<builtin>` |

---

### `rand(min, max)`

Return a random integer in the range `[min, max]` inclusive.

**Signature:** `rand(min: Int, max: Int) -> Int`
**Returns:** `Int`

Both arguments must be `Int` values.

```wilios
let n = rand(1, 6)    // simulates a die roll
let oct = rand(3, 5)  // random octave between 3 and 5
```

**Example — random melody:**

```wilios
let pitches_base = C4   // reference
let i = 0
loop (i < 8) {
    i = i + 1
    let semis = rand(0, 12)
    let note = transpose(C4, semis)
    <note> 1/8
}
```

---

### `transpose(pitch_or_chord, semitones)`

Transpose a pitch or chord by a given number of semitones.

**Signature:** `transpose(value: Pitch | Chord, semitones: Int) -> Pitch | Chord`
**Returns:** `Pitch` if given a `Pitch`; `Chord` if given a `Chord`

Positive semitones transpose up; negative semitones transpose down.

```wilios
let root  = C4
let fifth = transpose(C4, 7)     // G4
let octup = transpose(C4, 12)    // C5
let down  = transpose(A4, -12)   // A3

let maj = <C4, E4, G4>
let t   = transpose(maj, 5)      // <F4, A4, C5>
```

**Example — ascending scale:**

```wilios
let note = C4
let i = 0
loop (i < 8) {
    <note> 1/8
    note = transpose(note, 2)   // whole tone up each step
    i = i + 1
}
```

**Example — parallel harmony:**

```wilios
let melody = <C4, E4>
track 1
<melody> 1/4
<transpose(melody, 5)> 1/4     // parallel fourth up
```

---

### `len(array)`

Return the number of elements in an array.

**Signature:** `len(array: Array) -> Int`
**Returns:** `Int`

```wilios
let notes = [C4, E4, G4, B4]
let n = len(notes)   // 4
len([])              // 0
```

**Example — iterating over an array:**

```wilios
let scale = [C4, D4, E4, F4, G4, A4, B4]
let i = 0
loop (i < len(scale)) {
    <scale[i]> 1/8
    i = i + 1
}
```

---

## 2. Importing the Standard Library

Add this line at the top of your `.wilios` file to import all 9 preset functions:

```wilios
import "lib/lib.wilios"
```

Adjust the path to be relative to your source file's location. For example, from `examples/`:

```wilios
import "../lib/lib.wilios"
```

After importing, all preset functions (`epiano`, `brass`, `bass`, `marimba`, `strings`, `kick`, `snare`, `hihat_c`, `hihat_o`) are available in global scope and can be called from any track.

---

## 3. Preset Overview

| Function | Category | Algorithm | Operators | Recommended Pitch |
|----------|----------|-----------|-----------|-------------------|
| `epiano()` | Tonal | Two independent pairs | 4 | Any |
| `brass()` | Tonal | Single pair | 2 | Any |
| `bass()` | Tonal | Single pair | 2 | Low (A1–A2) |
| `marimba()` | Tonal | Two modulators → carrier | 3 | Mid–High |
| `strings()` | Tonal | Two independent pairs | 4 | Any |
| `kick()` | Drum | Single pair | 2 | `<B1>` |
| `snare()` | Drum | Two modulators → carrier | 3 | `<A3>` |
| `hihat_c()` | Drum | Two modulators → carrier | 3 | `<F5>` |
| `hihat_o()` | Drum | Two modulators → carrier | 3 | `<F5>` |

Each preset:
1. Sets the track's `wave` to `sine` (or `square` for snare)
2. Configures an `fm { }` block with operator routing and ADSR

The preset only affects the track in which it is called. Call it once per track before playing notes.

---

## 4. Tonal Presets

### 4.1 `epiano()`

**Electric piano.** Two independent 2-op pairs (DX7 Algorithm 5 style) produce the characteristic bell-like "ting" attack. High-ratio modulators (14×) create bright, fast-decaying inharmonic attacks. Both pairs are fully percussive — no sustain, long decay.

**Algorithm:**
```
Op 2 (14×, fast decay) ──> Op 1 (1×, slow decay) ──> \
                                                        ──> output
Op 4 (14×, fast decay) ──> Op 3 (1×, slow decay) ──> /
```

**Source:**
```wilios
let epiano = func() {
    wave sine
    fm {
        algorithm [2->1, 4->3]
        op 1 { ratio 1.0   level 0.5  attack 10  decay 1800  sustain 0   release 400 }
        op 2 { ratio 14.0  level 0.7  attack 0   decay 600   sustain 0   release 150 }
        op 3 { ratio 1.0   level 0.25 attack 10  decay 2500  sustain 0   release 500 }
        op 4 { ratio 14.0  level 0.4  attack 0   decay 400   sustain 0   release 100 }
    }
}
```

**Usage:**
```wilios
import "../lib/lib.wilios"

track 1
tempo 120
epiano()

<C4, E4, G4> 1/4
<F4, A4, C5> 1/4
<G4, B4, D5> 1/4
<C4, E4, G4> 1/2
```

---

### 4.2 `brass()`

**Brass stab.** A same-ratio modulator (1.0×) at high modulation depth (2.5) decays quickly, creating the bright "blat" attack that settles into a warm, sustained tone. Classic FM brass character.

**Algorithm:**
```
Op 2 (1×, high level, fast decay) ──> Op 1 (1×, full sustain) ──> output
```

**Source:**
```wilios
let brass = func() {
    wave sine
    fm {
        algorithm [2->1]
        op 1 { ratio 1.0  level 1.0  attack 25  decay 0    sustain 100  release 200 }
        op 2 { ratio 1.0  level 2.5  attack 10  decay 250  sustain 25   release 100 }
    }
}
```

**Usage:**
```wilios
track 1
tempo 100
brass()
attack 25
release 200

<C3> 1/8
<C3> 1/8
<G3> 1/4
<F3> 1/4
rest 1/4
```

---

### 4.3 `bass()`

**Deep FM bass.** A sub-octave modulator (ratio 0.5) at high depth produces a thick, punchy transient that quickly falls away, leaving a clean fundamental. The asymmetric decay (modulator decays faster than carrier) shapes the characteristic bass attack.

**Algorithm:**
```
Op 2 (0.5×, high level, fast decay) ──> Op 1 (1×, medium sustain) ──> output
```

**Source:**
```wilios
let bass = func() {
    wave sine
    fm {
        algorithm [2->1]
        op 1 { ratio 1.0  level 1.0  attack 5   decay 600  sustain 50  release 250 }
        op 2 { ratio 0.5  level 2.0  attack 0   decay 300  sustain 0   release 80  }
    }
}
```

**Usage:**
```wilios
track 1
tempo 116
bass()

rest 1/4
<A2> 3/4

rest 1/4
<D2> 3/4
```

---

### 4.4 `marimba()`

**Bell / mallet.** Two inharmonic modulators (3.5× and 5.0×) feed a single carrier. The inharmonic ratios produce metallic, resonant sidebands characteristic of struck metal or hard-mallet percussion. Fast decay, no sustain.

**Algorithm:**
```
Op 2 (3.5×) ──\
               ──> Op 1 (1×) ──> output
Op 3 (5.0×) ──/
```

**Source:**
```wilios
let marimba = func() {
    wave sine
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0  level 1.0  attack 5   decay 700  sustain 0  release 300 }
        op 2 { ratio 3.5  level 1.8  attack 0   decay 350  sustain 0  release 100 }
        op 3 { ratio 5.0  level 0.6  attack 0   decay 200  sustain 0  release 80  }
    }
}
```

**Usage:**
```wilios
track 1
tempo 140
marimba()

<C5> 1/8
<E5> 1/8
<G5> 1/8
<C6> 1/4
rest 1/8
<G5> 1/8
<E5> 1/8
<C5> 1/4
```

---

### 4.5 `strings()`

**Lush slow pad.** Two 2-op pairs with slow attacks produce a smooth swell. One pair covers the fundamental (ratio 1.0), the other adds an octave layer (ratio 2.0). Gentle modulation levels add harmonic warmth without brightness. Long release for sustained decay.

**Algorithm:**
```
Op 2 (1×, slow attack) ──> Op 1 (1×, slow attack) ──> \
                                                         ──> output
Op 4 (3×, slow attack) ──> Op 3 (2×, slow attack) ──> /
```

**Source:**
```wilios
let strings = func() {
    wave sine
    fm {
        algorithm [2->1, 4->3]
        op 1 { ratio 1.0  level 0.5  attack 500  decay 0  sustain 100  release 800  }
        op 2 { ratio 1.0  level 0.3  attack 300  decay 0  sustain 80   release 600  }
        op 3 { ratio 2.0  level 0.25 attack 600  decay 0  sustain 90   release 1000 }
        op 4 { ratio 3.0  level 0.2  attack 400  decay 0  sustain 70   release 700  }
    }
}
```

**Usage:**
```wilios
track 1
tempo 116
volume 50
strings()

<A3, C4> 4/4
<D3, C3> 4/4
<F3, A3> 4/4
<G3, D4> 4/4
```

> **Tip:** Because of the long attack (500–600ms), strings work best with sustained notes (`1/2`, `1/1`, `4/4`). Short notes will not reach full amplitude before decaying.

---

## 5. Drum Presets

Drum presets are tuned to specific pitch ranges for best results. The pitch you play them at affects the base frequency, which changes the timbre. Use the recommended pitch as a starting point and experiment.

### 5.1 `kick()`

**Bass drum.** A sub-octave modulator (ratio 0.25) at very high depth (9.0) creates the characteristic pitch-drop thud. The modulator decays extremely fast (60ms), producing a sharp, downward pitch sweep — the key feature of a kick drum. Best triggered at low notes.

**Algorithm:**
```
Op 2 (0.25×, very high level, very fast decay) ──> Op 1 (1×, fast decay) ──> output
```

**Recommended pitch:** `<B1>` (or similarly low notes)

**Source:**
```wilios
let kick = func() {
    wave sine
    fm {
        algorithm [2->1]
        op 1 { ratio 1.0   level 1.0  attack 5  decay 350  sustain 0  release 50 }
        op 2 { ratio 0.25  level 9.0  attack 0  decay 60   sustain 0  release 20 }
    }
}
```

**Usage:**
```wilios
track 1
tempo 120
kick()

let i = 0
loop (i < 4) {
    i = i + 1
    <B1> 1/4   // beat 1
    rest 1/4   // beat 2
    <B1> 1/4   // beat 3 (optional)
    rest 1/4   // beat 4
}
```

---

### 5.2 `snare()`

**Snare drum.** Two high-ratio inharmonic modulators (11× and 17×) on a square wave carrier produce a bright, noise-dense crack characteristic of a snare hit. The square wave adds odd harmonics; the inharmonic modulators create a dense, inharmonic spectrum mimicking white noise.

**Algorithm:**
```
Op 2 (11×, very high level, fast decay) ──\
                                            ──> Op 1 (1×, square, fast decay) ──> output
Op 3 (17×, high level, fast decay)     ──/
```

**Recommended pitch:** `<A3>` (or nearby mid-range notes)

**Source:**
```wilios
let snare = func() {
    wave square
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0   level 1.0  attack 0  decay 180  sustain 0  release 40 }
        op 2 { ratio 11.0  level 4.0  attack 0  decay 90   sustain 0  release 25 }
        op 3 { ratio 17.0  level 2.5  attack 0  decay 70   sustain 0  release 20 }
    }
}
```

**Usage:**
```wilios
track 1
tempo 120
snare()

rest 1/4    // beat 1 (no snare)
<A3> 1/4   // beat 2 (snare hit)
rest 1/4    // beat 3
<A3> 1/4   // beat 4 (snare hit)
```

---

### 5.3 `hihat_c()`

**Closed hi-hat.** Two very high-ratio inharmonic modulators (13× and 17.5×) produce a short, metallic click with a bright transient. The very short decays (30–55ms) make it sound crisp and tight — a closed hi-hat cuts off almost immediately.

**Algorithm:**
```
Op 2 (13×, very high level, very fast decay) ──\
                                                  ──> Op 1 (1×, very fast decay) ──> output
Op 3 (17.5×, high level, very fast decay)   ──/
```

**Recommended pitch:** `<F5>` (or other high-register notes)

**Source:**
```wilios
let hihat_c = func() {
    wave sine
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0   level 1.0  attack 0  decay 55   sustain 0  release 15 }
        op 2 { ratio 13.0  level 5.0  attack 0  decay 30   sustain 0  release 10 }
        op 3 { ratio 17.5  level 4.0  attack 0  decay 25   sustain 0  release 10 }
    }
}
```

**Usage:**
```wilios
track 1
tempo 120
hihat_c()

// Eighth-note hi-hat pattern
let i = 0
loop (i < 4) {
    i = i + 1
    <F5> 1/8
    <F5> 1/8
}
```

---

### 5.4 `hihat_o()`

**Open hi-hat.** Same topology as `hihat_c` but with much longer decay and release times (150–400ms). This creates the sustained "tsss" sound of an open hi-hat that continues to ring after being struck.

**Algorithm:**
```
Op 2 (13×, very high level, medium decay) ──\
                                              ──> Op 1 (1×, medium decay) ──> output
Op 3 (17.5×, high level, medium decay)   ──/
```

**Recommended pitch:** `<F5>` (or other high-register notes)

**Source:**
```wilios
let hihat_o = func() {
    wave sine
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0   level 1.0  attack 0  decay 400  sustain 0  release 200 }
        op 2 { ratio 13.0  level 5.0  attack 0  decay 180  sustain 0  release 80  }
        op 3 { ratio 17.5  level 4.0  attack 0  decay 150  sustain 0  release 60  }
    }
}
```

**Usage:**
```wilios
track 1
tempo 120

// Alternating closed and open hi-hat
hihat_c()
<F5> 1/8
<F5> 1/8
<F5> 1/8
hihat_o()
<F5> 1/8   // open hat — rings longer
```

> **Note:** Calling `hihat_o()` replaces the FM block on the track. If you want to alternate between closed and open hi-hat on the same track, call the preset function immediately before the note that should use it, as shown above.

---

## 6. Building Custom Presets

You can define your own preset functions following the same pattern as the standard library.

### Pattern

```wilios
let my_preset = func() {
    wave sine          // or square / saw / tri
    attack 10          // optional: set track ADSR
    decay 0
    sustain 100
    release 200
    fm {
        algorithm [...]
        op 1 { ... }
        op 2 { ... }
    }
}
```

### Example — Flute-Like Tone

```wilios
let flute = func() {
    wave sine
    attack 80
    decay 0
    sustain 90
    release 150
    fm {
        algorithm [2->1]
        op 1 { ratio 1.0  level 1.0 }
        op 2 { ratio 1.0  level 0.3  attack 200  decay 0  sustain 60  release 100 }
    }
}

track 1
tempo 120
flute()
<C5> 1/4
<D5> 1/4
<E5> 1/2
```

### Example — Clav / Harpsichord

```wilios
let clav = func() {
    wave square
    fm {
        algorithm [2->1]
        op 1 { ratio 1.0  level 1.0  attack 0   decay 500  sustain 0   release 80  }
        op 2 { ratio 8.0  level 1.2  attack 0   decay 100  sustain 0   release 30  }
    }
}
```

### Example — Sharing a Preset Across Tracks

Define the preset in global scope (before any `track` keyword) and call it from multiple tracks:

```wilios
import "../lib/lib.wilios"

let my_bell = func() {
    wave sine
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0  level 1.0  decay 1200  sustain 0  release 400 }
        op 2 { ratio 4.0  level 2.0  decay 600   sustain 0  release 200 }
        op 3 { ratio 7.0  level 1.0  decay 300   sustain 0  release 100 }
    }
}

track 1
tempo 120
my_bell()
<C5> 1/4
<E5> 1/4
<G5> 1/2

track 2
tempo 120
my_bell()      // same preset, independent track
<G4> 1/2
<C5> 1/1
```

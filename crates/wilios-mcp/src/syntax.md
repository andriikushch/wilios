# Wilios DSL Syntax Reference

Wilios is a music DSL that compiles and plays audio in real time. Source files use the `.wilios` extension.

## Tracks and Global Scope

```wilios
track 1      // switch to track 1 (independent playback thread)
track 2      // switch to track 2
global       // switch back to global scope
```

Statements before any `track`/`global` keyword go into global scope by default. Global-scope synth params act as defaults that a track can override locally.

## Tempo and Time

```wilios
tempo 120              // BPM (beats per minute)
time_signature 4/4     // numerator/denominator, default 4/4 (metadata only)
swing 50               // swing feel: 50 = straight, >50 = swing (e.g. 70, 90)
```

## Notes, Chords, and Rests

### Pitch notation
- Letter: `A`–`G`
- Accidental (optional): `#` (sharp) or `b` (flat)
- Octave: integer suffix (e.g. `C4`, `A#3`, `Bb5`)

### Duration notation
- `beats/division` format: `1/4` (quarter note), `1/8` (eighth), `1/2` (half), `1/1` (whole)
- Dotted: add `.` suffix — `1/4.` (dotted quarter), `1/8.` (dotted eighth)

### Syntax
```wilios
<C4> 1/4               // single note: pitch in angle brackets + duration
<C4, E4, G4> 1/2       // chord: multiple pitches separated by commas
rest 1/8               // rest
rest 1/8.              // dotted rest
```

## Variables and Assignment

```wilios
let i = 0              // variable declaration
i = i + 1              // assignment
let x = 3.14           // float literal
let flag = true        // boolean (true/false)
```

## Operators

- Arithmetic: `+`, `-`, `*`, `/`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Unary: `-` (negation)

## Control Flow

### Loop
```wilios
let i = 0
loop (i < 4) {
    i = i + 1
    <C4> 1/4
}
```
Condition is re-evaluated each iteration.

### If / Else
```wilios
if (i == 2) {
    <C4> 1/4
} else {
    <E4> 1/4
}
```
The `else` branch is optional.

## Functions

```wilios
let f = func(x) {
    return x
}
f(42)                  // function call as statement
let y = f(1)           // function call in expression
```

Functions are first-class values. `return` exits the function.

## Arrays

```wilios
let a = [C4, E4, G4]  // array literal (any value type: ints, pitches, chords, etc.)
a[0]                   // index (zero-based, must be Int)
a[0] = D4              // element assignment
len(a)                 // built-in: array length as Int
```

## Built-in Functions

```wilios
print(i)               // print values to stdout
rand(1, 10)            // random Int in [min, max] inclusive
transpose(<C4>, 7)     // transpose pitch or chord by semitones (returns same type)
len(a)                 // array length as Int
```

## Synth Parameters (per-track, all optional)

```wilios
wave sine              // waveform: sine (default), square, saw, tri
volume 80              // 0–127 (default 100)
pan -50                // -127 (left) to +127 (right), default 0
attack 10              // ADSR attack time in ms (default 10)
decay 0                // ADSR decay time in ms (default 0)
sustain 100            // ADSR sustain level 0–100 (default 100)
release 100            // ADSR release time in ms (default 100)
fm_ratio 1.0           // legacy 2-op FM: modulator freq = carrier * ratio (default 1.0)
fm_depth 0.0           // legacy 2-op FM: modulation index; 0 = no FM (default 0.0)
```

## Multi-Operator FM Block

Replaces the legacy `fm_ratio`/`fm_depth` for the track. Supports arbitrary N-op topologies.

```wilios
fm {
    algorithm [2->1, 3->1]   // routing: op 2 and op 3 modulate op 1 (carrier)
    op 1 { ratio 1.0  level 1.0  attack 10  decay 50  sustain 80  release 100 }
    op 2 { ratio 2.0  level 3.0 }
    op 3 { ratio 3.5  level 1.0  wave square }
}
```

- `algorithm` defines signal routing: `src->dst` means op `src` modulates op `dst`.
- Each `op` must have `ratio` and `level`; ADSR and `wave` are optional.
- Operators not listed as modulators in `algorithm` are carriers (their output goes to audio).
- Self-feedback is supported (one-sample delay).

## Import

```wilios
import "path/to/file.wilios"   // relative path; merges global_stmts and tracks
```

- Path must end in `.wilios` and be relative (no absolute paths).
- Circular imports are detected and rejected.
- Path must stay within the current working directory (no `../` escaping).

## Comments

```wilios
// This is a comment
```

## Identifiers

Identifiers must match `[a-z_][a-z_0-9]*` — start with a lowercase letter or underscore; digits allowed after the first character; uppercase letters are rejected.

## Complete Example

```wilios
import "lib/lib.wilios"

tempo 140

track 1
    epiano()
    volume 90
    let i = 0
    loop (i < 4) {
        <C4, E4, G4> 1/4
        <D4, F4, A4> 1/4
        i = i + 1
    }

track 2
    kick()
    volume 100
    let bar = 0
    loop (bar < 4) {
        <B1> 1/4
        rest 1/4
        <B1> 1/4
        rest 1/4
        bar = bar + 1
    }
```

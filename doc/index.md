# wilios — Music DSL

**wilios** is a domain-specific language for composing and playing music in real time. Source files use the `.wilios` extension. You write notes, chords, loops, and FM synthesis configuration in plain text; the runtime compiles and plays audio immediately.

---

## How it works

```
Source text (.wilios)
        │
        ▼
    [ Lexer ]  ──── tokens (pitches, durations, keywords, operators)
        │
        ▼
   [ Parser ]  ──── AST (Program, tracks, statements, expressions)
        │
        ▼
[ Interpreter ] ── pull-based scheduler, one TrackRunner per track
        │
        ▼
 [ Audio Engine ] ─ cpal callback, multi-voice FM synthesis, soft-clip
```

Each track runs independently and is driven by a shared wall-clock. Global statements (variables, function definitions) are evaluated once at startup and their results are cloned into every track's initial environment.

---

## Quick Start

```wilios
// Single-track hello world
tempo 120
volume 80

<C4> 1/4
<E4> 1/4
<G4> 1/4
<C5> 1/2
```

```wilios
// Two tracks playing simultaneously
let melody = func() {
    <C4> 1/4
    <E4> 1/4
    <G4> 1/2
}

track 1
tempo 120
melody()

track 2
tempo 120
volume 60
<C3> 1/1
```

Run with:

```
cargo run
```

Press **Enter** to stop playback.

---

## Documentation

| File                                           | Contents                                                                                                       |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| [language-reference.md](language-reference.md) | Complete language reference: syntax, types, operators, all statements, control flow, functions, scope, imports |
| [synthesis.md](synthesis.md)                   | Sound synthesis: waveforms, ADSR envelopes, performance controls, legacy 2-op FM, multi-operator FM blocks     |
| [stdlib.md](stdlib.md)                         | Standard library (`lib/lib.wilios`): 9 FM instrument presets with source, descriptions, and usage examples     |
| [grammar.ebnf](grammar.ebnf)                   | Formal ISO 14977 EBNF grammar                                                                                  |

---

## File Layout

```
.
├── examples/
│   └── example_1.wilios   # Demo composition using all major features
├── lib/
│   └── lib.wilios         # FM synthesis preset library (9 instruments)
├── doc/
│   ├── index.md           # This file
│   ├── language-reference.md
│   ├── synthesis.md
│   ├── stdlib.md
│   └── grammar.ebnf
└── crates/                # Rust implementation (Cargo workspace)
    ├── wilios-core/       # Lexer, parser, interpreter
    ├── wilios-synth/      # FM synthesis + voice mixer
    ├── wilios-cli/        # cpal audio engine + CLI (binary: wilios)
    └── wilios-mcp/        # Placeholder crate (not implemented yet)
```

---

## Key Concepts

**Tracks** — Independent sequencers. Use `track N` to switch scope to a numbered track. All tracks share global variables and functions defined before any `track` keyword.

**Pitches** — Written as uppercase letter + optional accidental + octave digit: `C4`, `F#5`, `Bb3`. Pitches are first-class values and can be stored in variables.

**Durations** — Written as `beats/division`, e.g. `1/4` (quarter note), `1/8.` (dotted eighth). Duration literals support the dotted suffix (1.5×); variable durations do not.

**FM Synthesis** — Each track has a full FM synthesis engine. Configure it with a `fm { ... }` block specifying operator routing and per-operator parameters.

**Arrays** — Ordered collections written as `[elem1, elem2, ...]`. Elements can be any type, including pitches and chords. Read with `a[i]`, write with `a[i] = expr`, and get the length with `len(a)`.

**Standard Library** — Import `lib/lib.wilios` to get 9 ready-made FM instrument presets (`epiano`, `brass`, `bass`, `kick`, `snare`, etc.).

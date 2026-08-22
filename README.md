[![CI](https://github.com/andriikushch/wilios/actions/workflows/ci.yml/badge.svg)](https://github.com/andriikushch/wilios/actions/workflows/ci.yml)

> **Early Version Notice**
> This is a very early version of wilios, created as part of an educational process. It is not production-ready — expect rough edges, missing features, and breaking changes. **Use at your own risk.** Feedback and contributions are very welcome — feel free to open an issue or reach out.

# wilios

A music DSL that interprets and plays audio in real time. Write melodies, chords, rhythms, and multi-track compositions in `.wilios` files — the interpreter runs the pipeline and plays sound immediately.

```
Source text (.wilios) → Lexer → Parser → AST → Interpreter → Events → Audio Engine
```

## Getting Started

**Prerequisites:** Rust (stable toolchain)

```bash
# Build
cargo build

# Run (plays audio — press Enter to quit)
cargo run

# Run the example composition
cargo run -- examples/example_1.wilios
```

## Quick Example

```wilios
import "../lib/lib.wilios"

track 1
tempo 120
epiano()

let i = 0
loop (i < 4) {
    i = i + 1
    <C4, E4, G4> 1/2
    rest 1/4
    <A4> 1/4
}

track 2
tempo 120
kick()

let j = 0
loop (j < 8) {
    j = j + 1
    <B1> 1/4
    rest 1/4
}
```

## Language at a Glance

| Construct       | Syntax                             |
| --------------- | ---------------------------------- |
| Single note     | `<C4> 1/4`                         |
| Chord           | `<C4, E4, G4> 1/2`                 |
| Dotted duration | `<A3> 1/2.`                        |
| Rest            | `rest 1/8`                         |
| Tempo           | `tempo 120`                        |
| Volume          | `volume 80`                        |
| Pan             | `pan -50`                          |
| Swing           | `swing 67`                         |
| Variable        | `let i = 0`                        |
| Assignment      | `i = i + 1`                        |
| Array literal   | `let a = [C4, E4, G4]`             |
| Array index     | `a[0]`                             |
| Array assign    | `a[0] = D4`                        |
| Loop            | `loop (i < 4) { ... }`             |
| Conditional     | `if (i == 2) { ... } else { ... }` |
| Function        | `let f = func(x) { return x }`     |
| Call            | `f(42)`                            |
| Import          | `import "path/to/file.wilios"`     |
| Track switch    | `track 1`                          |
| Global scope    | `global`                           |
| Comment         | `// text`                          |

Pitches use scientific notation: `C4`, `F#3` (sharp), `Bb4` (flat). Duration is `beats/division` (e.g. `3/8`).

**Built-ins:** `print(...)`, `rand(min, max)`, `transpose(pitch_or_chord, semitones)`, `len(array)`

**Waveforms:** `wave sine` | `wave square` | `wave saw` | `wave tri`

**Envelope:** `attack <ms>`, `decay <ms>`, `sustain <0–100>`, `release <ms>`

## Standard Library (`lib/lib.wilios`)

Import with `import "../lib/lib.wilios"` to access these FM synthesis presets:

| Function    | Description                    |
| ----------- | ------------------------------ |
| `epiano()`  | Electric piano                 |
| `brass()`   | Brass stab                     |
| `bass()`    | Bass synth                     |
| `marimba()` | Marimba                        |
| `strings()` | String pad                     |
| `kick()`    | Bass drum — play at `<B1>`     |
| `snare()`   | Snare drum — play at `<A3>`    |
| `hihat_c()` | Closed hi-hat — play at `<F5>` |
| `hihat_o()` | Open hi-hat — play at `<F5>`   |

## VS Code Extension

Syntax highlighting for `.wilios` files is provided by the bundled `vscode-wilios` extension.

**Install:**

```bash
make install-extension
```

This symlinks the extension into `~/.vscode/extensions/`. Reload the VS Code window afterwards (`Ctrl+Shift+P` → *Developer: Reload Window*).

**Regenerate the grammar** (after editing `doc/grammar.ebnf`):

```bash
make grammar
```

The TextMate grammar (`vscode-wilios/syntaxes/wilios.tmLanguage.json`) is generated from the EBNF by `tools/gen_grammar.py` — do not edit it by hand.

## Project Structure

```
src/
  lexer/          Tokenizer
  parser/         AST parser (Pratt expressions, two-token lookahead)
  interpreter/    Pull-based execution engine, track runners
  main.rs         cpal audio engine, voice mixer, FM synthesis
examples/         Example .wilios compositions
lib/              Standard FM preset library (lib.wilios)
doc/              Language reference, stdlib docs, synthesis notes, grammar
tests/            Integration tests, property-based tests
fuzz/             libfuzzer targets for lexer, parser, interpreter
tools/            Development tools (gen_grammar.py — EBNF → tmLanguage.json)
vscode-wilios/    VS Code syntax highlighting extension
```

## Testing

```bash
# All tests
cargo test

# Property-based tests
cargo test --test fuzz_props

# Fuzz targets (requires nightly + cargo-fuzz)
cargo +nightly fuzz run fuzz_lexer
cargo +nightly fuzz run fuzz_parser
cargo +nightly fuzz run fuzz_interpreter
```

## Documentation

- [Language Reference](doc/language-reference.md) — full syntax and semantics
- [Standard Library](doc/stdlib.md) — built-in functions and FM presets
- [Synthesis](doc/synthesis.md) — FM synthesis details (2-op legacy and multi-op blocks)
- [Grammar](doc/grammar.ebnf) — EBNF grammar specification

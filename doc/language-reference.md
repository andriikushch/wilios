# wilios Language Reference

This document is the complete reference for the wilios music DSL. For synthesis-specific details see [synthesis.md](synthesis.md). For the standard library see [stdlib.md](stdlib.md).

---

## Table of Contents

1. [Lexical Rules](#1-lexical-rules)
2. [Types](#2-types)
3. [Literals](#3-literals)
4. [Expressions and Operators](#4-expressions-and-operators)
5. [Statements](#5-statements)
   - [Musical Statements](#51-musical-statements)
   - [Performance Control](#52-performance-control)
   - [Variables](#53-variables)
   - [Control Flow](#54-control-flow)
   - [Functions](#55-functions)
   - [Scope Switching](#56-scope-switching)
6. [Duration Syntax](#6-duration-syntax)
7. [Pitch Notation](#7-pitch-notation)
8. [Chord Syntax](#8-chord-syntax)
9. [Arrays](#9-arrays)
10. [Import System](#10-import-system)
11. [Scope Model](#11-scope-model)
12. [Execution Model](#12-execution-model)
13. [Reserved Keywords](#13-reserved-keywords)
14. [Known Limitations](#14-known-limitations)

---

## 1. Lexical Rules

### Comments

Single-line comments start with `//` and run to the end of the line. They are stripped by the lexer and produce no tokens.

```wilios
// This is a comment
<C4> 1/4   // inline comment
```

Block comments are not supported.

### Newlines

Newlines are **tokenized** by the lexer (they are not silently discarded like spaces). The parser skips newline tokens at statement boundaries, so a newline between statements is **not required** — multiple statements on the same line are valid.

What newlines do prevent is a statement **spanning** multiple lines: a newline encountered in the middle of an expression causes a parse error.

```wilios
// These are equivalent — newline between statements is optional
tempo 120
volume 80

tempo 120 volume 80
```

```wilios
// Parse error — expression cannot span lines
let x = 1 +
2
```

Blank lines and multiple consecutive newlines are silently skipped.

### Identifiers

Identifiers must match the pattern `[a-z][a-z_0-9]*`:

- Must start with a **lowercase letter** (a–z)
- Subsequent characters may be lowercase letters, underscores (`_`), or digits (`0`–`9`)
- A **leading underscore or digit** is not allowed as the first character
- Uppercase letters are **not** allowed (they are reserved for pitch notation)

```wilios
// Valid identifiers
melody
my_func
hi
track2
voice_1
step10

// Invalid — would be rejected by the lexer
MyFunc        // uppercase
_helper       // leading underscore
2voice        // leading digit (parsed as Int 2, then ident "voice")
```

### String Literals

Strings are delimited by double quotes and support the following escape sequences:

| Escape | Meaning |
|--------|---------|
| `\n` | Newline |
| `\t` | Horizontal tab |
| `\\` | Literal backslash |
| `\"` | Literal double quote |

```wilios
import "lib/lib.wilios"
print("hello\nworld")
```

---

## 2. Types

wilios has seven runtime value types:

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit signed integer | `42`, `-7` |
| `Float` | 32-bit floating point | `2.0`, `0.5` |
| `Bool` | Boolean | `true`, `false` |
| `Pitch` | A musical note (letter + accidental + octave) | `C4`, `F#5` |
| `Chord` | An ordered list of pitches | `<C4, E4, G4>` |
| `Array` | Ordered heterogeneous collection | `[1, 2, 3]`, `[C4, E4]` |
| `Func` | User-defined function value | `func(x) { return x }` |
| `Builtin` | Native built-in function | `print`, `rand`, `transpose`, `len` |

### Type behaviour

- **Int** and **Float** are distinct. Arithmetic between them is not implicitly coerced — use explicit literals of the correct type.
- **Pitch** and **Chord** are first-class values that can be stored in variables, passed to functions, and returned.
- **Array** elements can be any value type, including pitches and chords. Arrays are indexed from zero.
- **Func** values are closures that capture their definition-time environment.

---

## 3. Literals

### Integers

A sequence of decimal digits: `0`, `1`, `120`, `65535`.

### Floats

Digits, a dot, then more digits: `1.0`, `0.5`, `3.14`. Both sides of the dot are required (`1.` and `.5` are not valid).

### Booleans

`true` and `false` (lowercase, reserved keywords).

### Pitches

See [Section 7 — Pitch Notation](#7-pitch-notation).

### Durations

See [Section 6 — Duration Syntax](#6-duration-syntax).

---

## 4. Expressions and Operators

Expressions are parsed with a Pratt (top-down operator-precedence) parser.

### Operator Precedence

Precedence levels from **lowest** to **highest**:

| Level | Operator(s) | Associativity | Description |
|-------|------------|---------------|-------------|
| 1 | `\|\|` | Left | Logical OR |
| 2 | `&&` | Left | Logical AND |
| 3 | `==` `!=` | Left | Equality |
| 4 | `<` `<=` `>` `>=` | Left | Relational |
| 5 | `+` `-` | Left | Additive |
| 6 | `*` `/` `%` | Left | Multiplicative |
| 7 | `f(...)` `a[i]` | Left | Function call / array index (postfix) |

### Unary Operators

| Operator | Meaning | Example |
|----------|---------|---------|
| `-` | Arithmetic negation | `-x`, `-42` |
| `+` | Identity (no-op) | `+x` |

> **Note:** The logical NOT operator `!` is defined in the language's internal AST and token set but is **not parseable** in wilios source code. Use De Morgan's laws or restructure conditions as a workaround.

### Arithmetic

```wilios
let a = 10 + 3    // 13
let b = 10 - 3    // 7
let c = 10 * 3    // 30
let d = 10 / 3    // 3  (integer division when both operands are Int)
let e = 10 % 3    // 1  (remainder)
```

### Comparison

```wilios
let eq  = 1 == 1    // true
let neq = 1 != 2    // true
let lt  = 1 < 2     // true
let lte = 2 <= 2    // true
let gt  = 3 > 1     // true
let gte = 3 >= 3    // true
```

### Logical

```wilios
let and = true && false   // false
let or  = true || false   // true
```

### Grouping

Use parentheses to override precedence:

```wilios
let x = (2 + 3) * 4   // 20
```

### Function Call Expressions

A function stored in a variable is called using postfix call syntax. The result is a value that can be used in further expressions.

```wilios
let double = func(x) { return x * 2 }
let y = double(21)       // 42
let z = double(double(5)) // 20
```

Built-in functions are also called this way:

```wilios
let n   = rand(1, 10)
let t   = transpose(<C4>, 7)
```

### Chord Expressions

A chord expression (without a duration) produces a `Chord` value:

```wilios
let triad = <C4, E4, G4>
```

### Array Expressions

An array literal produces an `Array` value. Elements can be any expression, including pitches and chords:

```wilios
let nums   = [1, 2, 3]
let notes  = [C4, E4, G4]
let chords = [<C4, E4, G4>, <D4, F4, A4>]
let mixed  = []          // empty array
```

Array elements are read with postfix `[index]`:

```wilios
let x = nums[0]          // 1
let p = notes[2]         // G4
```

---

## 5. Statements

Statements are separated by newlines. A statement cannot span multiple lines.

### 5.1 Musical Statements

#### Note / Chord

Play one or more pitches simultaneously for a given duration.

```
<expr, expr, ...> duration
```

A single pitch in angle brackets is a one-note "chord":

```wilios
<C4> 1/4          // single note, quarter duration
<C4, E4, G4> 1/2  // three-note chord, half duration
<A4> 1/8.         // dotted eighth note
```

The pitch expressions inside `< >` can be any expression that evaluates to a `Pitch` or (for multi-note chords) the entire expression after `>` must be a duration. Chord values from variables can be played:

```wilios
let chord = <C4, E4, G4>
<chord> 1/4   // plays the chord
```

#### Rest

Silence for a given duration:

```
rest duration
```

```wilios
rest 1/4    // quarter rest
rest 1/8.   // dotted eighth rest
rest 1/1    // whole rest
```

---

### 5.2 Performance Control

These statements take effect immediately and persist for all subsequent notes on the same track until changed.

#### Tempo

Set the playback speed in beats per minute. Accepts a literal integer only.

```
tempo integer
```

```wilios
tempo 120    // 120 BPM
tempo 80     // slower
tempo 200    // faster
```

#### Volume

Set the track volume. Range: `0` (silent) to `127` (loudest). Accepts a literal integer only.

```
volume integer
```

```wilios
volume 100   // default
volume 60    // quieter
volume 0     // mute
```

#### Pan

Set the stereo position. Range: `-127` (full left) to `+127` (full right). `0` is centre. Accepts a literal integer (optionally preceded by `-`).

```
pan integer
pan -integer
```

```wilios
pan 0       // centre (default)
pan -64     // left of centre
pan 127     // hard right
```

#### Swing

Apply a swing rhythmic feel to 8th-note pairs. The on-beat (even) 8th is lengthened; the off-beat (odd) 8th is shortened. Quarter notes and larger values are unaffected; notes shorter than an 8th pass through unchanged. Accepts an integer or float literal.

```
swing integer_or_float
```

Valid range: `50` (straight, default) to `100` (maximum swing). Values outside this range are a runtime error.

```wilios
tempo 120
swing 67     // classic jazz swing: 8ths play as 335ms + 165ms
<C4> 1/8    // on-beat → 335ms
<D4> 1/8    // off-beat → 165ms
<E4> 1/4    // quarter note → 500ms (always unchanged)

swing 50     // back to straight
```

See [synthesis.md — Swing](synthesis.md#swing) for a detailed description and examples.

---

### 5.3 Variables

#### Declaration

Declare a new variable and bind it to a value. The variable must not already exist in the current scope.

```
let ident = expr
```

```wilios
let i = 0
let name = "hello"
let f = func(x) { return x * 2 }
let note = C4
let chord = <C4, E4, G4>
```

#### Assignment

Reassign an existing variable. The variable must have been declared with `let` first.

```
ident = expr
```

```wilios
i = i + 1
name = "world"
```

---

### 5.4 Control Flow

#### Loop

Execute a block repeatedly while a condition is true. The condition is re-evaluated before **each** iteration.

```
loop (expr) {
    stmt
    ...
}
```

```wilios
let i = 0
loop (i < 4) {
    i = i + 1
    <C4> 1/4
}
```

An infinite loop (be careful — there is no break statement):

```wilios
let running = true
loop (running) {
    <C4> 1/4
    // no way to exit without external termination
}
```

> **Note:** There is no `break` or `continue` statement. Design loops with care.

#### If / Else

Conditionally execute one of two blocks.

```
if (expr) {
    stmt
    ...
}

if (expr) {
    stmt
    ...
} else {
    stmt
    ...
}
```

```wilios
let i = rand(0, 1)

if (i == 0) {
    <C4> 1/4
} else {
    <G4> 1/4
}
```

Blank lines between `}` and `else` are allowed:

```wilios
if (x > 0) {
    <E4> 1/4

} else {
    <D4> 1/4
}
```

`else if` is achieved by nesting:

```wilios
if (x == 0) {
    <C4> 1/4
} else {
    if (x == 1) {
        <E4> 1/4
    } else {
        <G4> 1/4
    }
}
```

---

### 5.5 Functions

#### Function Literal

A function value is created with the `func` keyword:

```
let ident = func(param, ...) {
    stmt
    ...
}
```

```wilios
let greet = func() {
    print("hello")
}

let add = func(a, b) {
    return a + b
}

// Functions can be used as values
let apply = func(f, x) {
    return f(x)
}
```

Parameters and the function body are lexically scoped — the function captures its enclosing environment at definition time.

#### Return

Return a value from the current function. Unwinds all inner frames (nested loops, if branches) back to the nearest function call boundary.

```
return expr
```

```wilios
let max = func(a, b) {
    if (a > b) {
        return a
    }
    return b
}
```

A function with no explicit `return` implicitly returns `Int(0)`.

#### Call Statement

Call a function as a statement (discarding the return value). This is the form used to invoke musical preset functions and other side-effectful functions.

```
ident(expr, ...)
```

```wilios
greet()
strings()      // FM preset from stdlib
verse()        // user-defined musical phrase
print(x, y)   // built-in
```

#### Call Expression

Call a function inside an expression (the return value is used):

```wilios
let n = rand(1, 10)
let shifted = transpose(<C4>, 5)
let result = add(1, 2) * 3
```

---

### 5.6 Scope Switching

#### Track

Switch the current statement scope to a numbered track. Subsequent statements go into that track's sequencer until `global` or another `track N` is encountered. Tracks are created on first reference.

```
track integer
```

```wilios
track 1
tempo 120
<C4> 1/4

track 2
tempo 120
<G3> 1/4
```

#### Global

Switch the current statement scope back to global. Subsequent statements are added to the global statement list.

```
global
```

```wilios
track 1
<C4> 1/4

global
let shared = func() {
    <G4> 1/4
}

track 2
shared()
```

---

## 6. Duration Syntax

A duration specifies how long a note, chord, or rest lasts. It is always written as a fraction.

### Literal Duration (Path A)

When both the numerator and denominator are integer literals written directly as `beats/division`, the lexer tokenizes the entire form as a single `Duration` token. The optional trailing `.` makes the duration **dotted** (1.5×).

```
integer/integer[.]
```

| Example | Meaning |
|---------|---------|
| `1/1` | Whole note |
| `1/2` | Half note |
| `1/4` | Quarter note |
| `1/8` | Eighth note |
| `1/16` | Sixteenth note |
| `1/4.` | Dotted quarter (= 3/8) |
| `1/8.` | Dotted eighth (= 3/16) |
| `3/4` | Three-quarter note (dotted half) |
| `4/4` | Whole measure at 4/4 |

```wilios
<C4> 1/4
<D4> 1/8
<E4> 1/4.
rest 1/2
```

### Variable Duration (Path B)

When one or both of the numerator/denominator are variable names, the duration is parsed from individual tokens. The dotted suffix is **not** available in this form.

```
(ident | integer) / (ident | integer)
```

```wilios
let beats = 1
let div   = 4
<C4> beats/div

let d = 8
<G4> 1/d
```

> **Tip:** Use literal durations (`1/4`, `1/8.`) whenever possible. Variable durations exist for algorithmic composition where durations are computed at runtime.

---

## 7. Pitch Notation

A pitch is an uppercase letter, an optional accidental, and a single-digit octave number.

```
letter [accidental] octave
```

| Component | Values |
|-----------|--------|
| Letter | `A` `B` `C` `D` `E` `F` `G` |
| Accidental | `#` (sharp, +1 semitone) or `b` (flat, −1 semitone) |
| Octave | `0` through `9` |

```wilios
C4    // middle C
A4    // concert A (440 Hz)
F#5   // F sharp, octave 5
Bb3   // B flat, octave 3
G0    // very low G
D9    // very high D
```

**Pitch as a value:** Pitches are first-class values and can be stored in variables or passed to functions:

```wilios
let root = C4
let fifth = G4

<root> 1/4
<fifth> 1/4

let shifted = transpose(root, 7)  // G4
<shifted> 1/4
```

**Enharmonic equivalents:** `C#4` and `Db4` are two spellings of the same frequency. The language does not normalise them — both are valid and produce the same audio output.

---

## 8. Chord Syntax

A chord is a list of pitch expressions inside angle brackets.

### Chord Statement (with duration)

```
<expr, expr, ...> duration
```

All pitches sound simultaneously for the given duration.

```wilios
<C4, E4, G4> 1/4         // C major triad, quarter note
<A3, C4, E4> 1/2         // A minor triad, half note
<C4, E4, G4, B4> 1/4.    // Cmaj7, dotted quarter
```

### Chord Expression (without duration)

A chord without a duration produces a `Chord` value:

```wilios
let maj = <C4, E4, G4>
let min = <A3, C4, E4>
```

The value can later be played, transposed, or passed to a function:

```wilios
let maj = <C4, E4, G4>
<maj> 1/2

let shifted = transpose(maj, 5)   // F major
<shifted> 1/2
```

### Single-Note "Chord"

A single pitch in angle brackets is syntactically a one-note chord. Both forms are equivalent:

```wilios
<C4> 1/4       // plays C4 for a quarter note
```

---

## 9. Arrays

Arrays are ordered collections that can hold any value type — integers, booleans, pitches, chords, or even other arrays.

### Array literal

```wilios
let nums   = [1, 2, 3]
let notes  = [C4, E4, G4, B4]
let chords = [<C4, E4, G4>, <D4, F4, A4>]
let empty  = []
```

### Reading elements

Use postfix `[index]` (zero-based) to read an element:

```wilios
let first = nums[0]      // 1
let third = notes[2]     // G4
```

The index must be an `Int`. Accessing an out-of-bounds index is a runtime error.

### Writing elements

Use `name[index] = expr` to update an element in place:

```wilios
notes[0] = D4
nums[2] = nums[2] + 10   // 13
```

### `len` built-in

`len(array)` returns the number of elements as an `Int`:

```wilios
let n = len(notes)   // 4
```

### Iterating with a loop

```wilios
let notes = [C4, E4, G4, B4]
let i = 0
loop (i < len(notes)) {
    <notes[i]> 1/4
    i = i + 1
}
```

### Pitch and chord arrays

Array elements that are pitches or chords can be used directly in chord statements:

```wilios
let scale = [C4, D4, E4, F4, G4, A4, B4]
<scale[0]> 1/4    // plays C4

let progression = [<C4, E4, G4>, <F4, A4, C5>, <G4, B4, D5>]
<progression[1]> 1/2    // plays F major triad
```

### `print` format

`print` renders arrays as `[elem1, elem2, ...]`:

```wilios
print([1, 2, 3])          // [1, 2, 3]
print([C4, E4])           // [C4, E4]
print([<C4, E4>, <D4>])   // [<C4, E4>, <D4>]
```

---

## 10. Import System

The `import` statement merges another `.wilios` file's global statements and tracks into the current program.

```
import "path/to/file.wilios"
```

- Paths are **relative to the importing file's directory**.
- The path must end in `.wilios` — any other extension is rejected.
- The path must be relative — absolute paths (e.g. `import "/etc/passwd"`) are rejected.
- The resolved file must stay within the current working directory (the project root) — `..` traversal that would escape it is rejected.
- Circular imports are detected and silently skipped — a file is only imported once regardless of how many times it is referenced.
- Duplicate imports of the same canonical path are skipped.
- Imported global statements (variable and function definitions) become available in the importing file's global scope.
- Imported tracks are merged into the program's track list.

```wilios
import "../lib/lib.wilios"

track 1
strings()
<C4, E4, G4> 1/1
```

Import statements must appear at the top level (not inside a block, loop, or function body).

---

## 11. Scope Model

### Global Scope

Statements written before any `track` keyword, and statements written after a `global` keyword, belong to global scope. Global statements are evaluated **once** at program startup. The resulting environment (variables and function bindings) is **cloned** into every track's initial environment.

This means functions and variables defined globally are available in all tracks.

```wilios
// Global scope — available in all tracks
let bpm = 120
let chord_i = <C4, E4, G4>

let verse = func() {
    <chord_i> 1/4
    <chord_i> 1/4
}

track 1
tempo bpm
verse()
```

### Track Scope

Each `track N` block has its own independent state:
- Wall-clock time (each track advances independently)
- Tempo, volume, pan
- Synthesis parameters (waveform, ADSR, FM)
- Variables declared inside the track

Tracks do **not** share mutable state with each other. A variable mutated in track 1 does not affect track 2.

### Variable Lookup

Inside a track, variable lookup searches:
1. The track's own environment (variables declared with `let` inside the track)
2. The cloned global environment

There is no dynamic parent-scope lookup across tracks.

### Function Scope

When a function is called, its execution frame captures the environment at the time of **definition** (lexical scoping). The function body executes with that captured environment. Mutations inside the function body (assignments) affect the track's current environment, not the capture. Parameters shadow any outer variable with the same name.

---

## 12. Execution Model

wilios uses a **pull-based scheduler**. The audio engine asks the interpreter for events within a time window (`schedule_until(from_ms, until_ms)`). Each track advances its execution stack until it either exhausts events in the window or runs out of statements.

### Execution Stack Frames

Each track maintains a stack of execution frames:

| Frame Type | Description |
|------------|-------------|
| `Block` | Top-level or if-branch body |
| `Loop` | Saved condition + body + program counter |
| `FunctionCall` | Function body + program counter; environment saved/restored |

### Multi-track Parallelism

Tracks run logically in parallel but are interleaved by the scheduler. Each track has its own:
- Execution stack
- Time cursor
- Synthesis state

Tracks do not communicate at runtime.

### Global Initialisation

1. Parse all imported files (merged into the program).
2. Execute `global_stmts` in order.
3. Clone the resulting `env_vars` into every track's initial environment.
4. Begin the audio loop; tracks are scheduled pull-based.

---

## 13. Reserved Keywords

The following identifiers are reserved and **cannot** be used as variable or function names:

| Category | Keywords |
|----------|----------|
| Control flow | `loop` `if` `else` `return` |
| Variables / functions | `let` `func` |
| Scope | `track` `global` |
| Musical | `tempo` `volume` `pan` `rest` |
| Synthesis | `wave` `attack` `decay` `sustain` `release` `fm_ratio` `fm_depth` `swing` |
| FM block | `fm` `op` `algorithm` `level` `ratio` |
| Waveforms | `sine` `square` `saw` `tri` |
| Import | `import` |
| Literals | `true` `false` |

Pitch letters (`A`–`G`) are uppercase and therefore distinct from lowercase identifiers — they cannot be confused with keywords.

---

## 14. Known Limitations

### No Logical NOT operator

The `!` operator token and `UnaryOp::Not` AST variant exist internally but the parser does not handle `!expr` as a primary expression. You cannot write `!condition` in wilios source code.

**Workaround:** Restructure conditions:

```wilios
// Instead of: if (!done) { ... }
if (done == false) { ... }

// Instead of: loop (!finished) { ... }
loop (finished == false) { ... }
```

### No Break / Continue

There is no `break` or `continue` statement. A loop always runs until its condition becomes false.

### No Else-If shorthand

`else if` must be written as a nested `if` inside the `else` block:

```wilios
if (x == 0) {
    <C4> 1/4
} else {
    if (x == 1) {
        <E4> 1/4
    } else {
        <G4> 1/4
    }
}
```

### Variable Durations Cannot Be Dotted

The dotted suffix (`.`) is only available for literal durations tokenized at lex time. Variable durations (`beats/div`) do not support the dot.

### No Per-Note Synthesis Overrides

Waveform, ADSR envelope, and FM parameters are per-track. All notes on a track share the same synthesis settings at the moment they are scheduled.

### No Multi-line Statements

Every statement must fit on a single line. There is no line-continuation syntax.

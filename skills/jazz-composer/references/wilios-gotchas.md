# wilios gotchas for composers

Every DSL limitation that bites while composing, with the workaround. The
canonical patterns are in `examples/bebop_trio.wilios` and
`examples/blues_f.wilios`.

## `func` scoping is call-site (dynamic), and bindings don't escape

A `func` body sees its own parameters (they shadow) on top of whatever is in
scope **where it is called** — so it *can* call other top-level `func`s,
presets, and the builtins `print` / `rand` / `transpose` / `len`, and `func`s
may recurse or be mutually recursive.

- **`func` calling a `func` works.** Build `kit()` out of `kick()` / `snare()`,
  or a bar out of sub-phrases — as long as the callees are defined at global
  scope. (`bebop_trio.wilios` track 4 and `blues_f.wilios` still show the
  hand-inlined form; the helper form is now allowed.)
- **`transpose` / `rand` inside a phrase `func` works**, but the result is local
  to that body — a `let` or parameter bound inside a `func` is discarded when the
  call returns. Precompute-and-pass is still fine and keeps `rand` reproducible.

  ```wilios
  // both work now
  let play3 = func(a, b, c) { <a> 1/8 <b> 1/8 <c> 1/8 }
  let lick  = func(root) { play3(root, transpose(root, 4), transpose(root, 7)) }
  // ...inside a track:
  lick(C4)
  ```

- **Keep helper and phrase `func`s at global scope.** Because resolution is
  call-site, a track-local `let step = …` shadows a global `func step` for
  anything invoked from that track — give track variables non-colliding names.
- Deep runaway recursion in expression position (`let y = f()` with no base
  case) stops with a `call stack too deep` runtime error rather than crashing.

## `tempo` / `swing` / `time_signature` are per-track

- `tempo` accepts a **literal integer** only — no variable, no expression, no
  `tempo bpm`. (`volume`, `pan` are also literal-int only.) `swing` and the
  ADSR params *do* take expressions, so `let feel = 63` … `swing feel` keeps
  one feel for every track and every phrase `func` that restores it.
- These settings do **not** propagate between tracks. Restate `tempo`, `swing`,
  and `time_signature` at the top of **every** `track` block with the same
  values, or the tracks run at different rates and drift apart.
- Setting them in `global` scope makes them the default for every track — a
  clean way to keep them in sync — but a track that sets its own overrides the
  default, so be consistent.

## Chords are explicit pitch sets

No chord symbols, no `chord("G7")`, no roman numerals, no `scale()`, no degree
notation (`[1 3 5 b7]`). Write the symbol as a `//` comment and spell every
sounding note: `<G2, F3, B3, E4> 1/2`. `transpose(<chord>, n)` is the only
transform (it works anywhere, inside a `func` body included).

## No per-note expression

Velocity == the track `volume` for every note on that track. There is no accent,
ghost note, articulation, or per-note dynamic. Build contrast from register,
rhythm, density, rests, `pan`, and which preset plays the line.

## Pitch grammar

`LETTER[#|b]OCTAVE` — letter `A`–`G` (uppercase), one accidental max, octave
`0`–`9`. `C4` = middle C = MIDI 60. `C#4` and `Db4` are the same pitch, not
normalized — pick spellings that match the harmony (`F#` in a D7 bar, `Gb` as a
descending approach). No double sharps/flats.

## Control flow

- No `break`, no `continue` — a `loop` runs until its condition is false. Design
  the iteration count (`loop (i < 12) { i = i + 1  ... }`).
- No `else if` — nest `if` inside `else`.
- No unary `!` operator — write `done == false`. The binary `!=` comparison
  *does* work, though: `loop (i != 12)`, `if (x != y)`.
- `loop (true) { ... }` never terminates, which forces `--duration S` on
  `dump` / `render` / `midi`. A bounded counter loop does not.

## Durations

- `beats/division` in whole-note units: `1/4` = quarter, `1/8.` = dotted 8th
  (= `3/16`). Dotted form works **only** on literal durations.
- Variable durations (`n/d` where `n` or `d` is a variable) are allowed but
  cannot be dotted, and a runtime-computed division that is implausibly large is
  a `TimeError` naming the line.
- **Under `swing`, every duration must be a whole multiple of `1/8`.** Values of
  `1/8` or longer are re-quantized to whole 8th slots, so a dotted 8th (`3/16`)
  or a `1/6` quarter triplet comes out wrong and the bar drifts; shorter values
  pass through but leave the position off the grid. Full rule:
  [`doc/synthesis.md`](../../../doc/synthesis.md#what-swing-does-to-a-duration).
- Tuplets are exact and never drift **at straight feel**: `1/12` = 8th-note
  triplet, `1/6` = quarter triplet, `1/20` = quintuplet in a quarter.
- **`1/16` (and shorter non-tuplet) notes under `swing` drift the bar's exact
  position** — a phrase of `1/16 1/16 1/8` groups repeated under `swing 63` ends
  a hair short of a whole bar, so a `smoke` equal-length check fails. Tuplets
  *shorter than an 8th* (`1/12`, `1/20`) do not drift — they take swing's early
  return. For a bebop enclosure, write the pickup as an 8th-note triplet
  (`<x> 1/12 <y> 1/12 <t> 1/12`), not `1/16`s.
- **A `1/6` quarter triplet is longer than an 8th, so `swing` *does* mangle
  it**: each note is rounded to one swung 8th slot, and the three together come
  out ~1.6 beats instead of 2 — the bar is short and the tracks desync. Quarter
  triplets are played even anyway, so straighten the feel around them and
  restore it (`swing` takes an expression, so the feel can be a variable):

  ```wilios
  let feel = 63
  let trip = func(a, b, c) {
      swing 50
      <a> 1/6 <b> 1/6 <c> 1/6
      swing feel
  }
  ```

  Worked example: `examples/all_of_me.wilios`.
- `time_signature` never changes a note's length — a `1/8` is the same in 4/4
  and 7/8. It only anchors the swing bar-phase and is stamped as metadata.

## Randomness is not reproducible

`rand(min, max)` is unseeded — every `render` of a piece that calls it sounds
different. For a fixed piece, don't use `rand`; write the notes.

## Presets

The 14 presets (from `import`ing `lib/lib.wilios`) are the tonal `brass`,
`trumpet`, `epiano`, `bass`, `upright`, `marimba`, `strings`, `comp_piano` and
the drums `kick`, `snare`, `hihat_c`, `hihat_o`, `ride`, `brushes`. `trumpet` is
a sustained brass lead with vibrato, distinct from `brass`'s short stab;
`upright` is a rounder fingered acoustic bass; `comp_piano` holds a real sustain
for held comping chords (`epiano` is purely percussive); `ride` is a sustained
cymbal wash, distinct from `hihat_c`'s short click; `brushes` is a soft swish
rather than `snare`'s crack. For anything outside that set, define a custom patch at global
scope:

```wilios
let vibes = func() {
    wave sine
    fm {
        algorithm [2->1, 3->1]
        op 1 { ratio 1.0  level 1.0  attack 0  decay 900  sustain 0  release 400 }
        op 2 { ratio 4.2  level 3.5  attack 0  decay 300  sustain 0  release 120 }
        op 3 { ratio 6.7  level 2.0  attack 0  decay 250  sustain 0  release 100 }
    }
}
```

Calling a preset again on the same track **replaces** its FM block — used
deliberately in `bebop_trio.wilios` to alternate `hihat_c` / `hihat_o`, and to
run `kick` / `snare` off one track.

## Import paths

`import "<path>/lib.wilios"` is resolved **relative to the importing file** and
must stay inside the process's current working directory. From the repo
`examples/` dir — where both `blues_f.wilios` and `bebop_trio.wilios` live —
that's `../lib/lib.wilios`. Count the `../` from wherever you actually put the
file.

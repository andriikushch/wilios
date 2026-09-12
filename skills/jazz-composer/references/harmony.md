# Harmony

## Functional harmony

Use functional relationships deliberately — but they are not mandatory
ingredients. Common structures:

```
ii–V–I                         Dm7  G7   Cmaj7
iii–VI–ii–V (turnaround)       Em7  A7   Dm7  G7
tritone sub                    Dm7  Db7  Cmaj7        (Db7 for G7)
backdoor dominant              Fm7  Bb7  Cmaj7        (bVII7 → I)
secondary dominant             A7 → Dm7 (V/ii)
dominant chain                 E7  A7   D7   G7  C
minor ii–V                     Dm7b5  G7b9  Cm
```

Harmonic rhythm matters as much as the chords: two bars per chord vs. two chords
per bar changes the whole feel. Put strong cadences at section ends; delay or
sidestep them mid-section to keep motion.

## Modal harmony

Distinguish static modality (one center, color from voicing/register/melody),
modal interchange (borrowing `iv`, `bVI`, `bVII` from the parallel minor),
pedal harmony, and planed (parallel) chords. If the harmony is static, motion
has to come from **rhythm, register, melody, and orchestration** — say so and
build it there.

## Chromatic harmony

A chromatic chord needs a perceivable role: approach, passing, dominantization,
side-slip, chromatic mediant, voice-leading chord, or pure color. If you can't
name its function, it probably doesn't belong.

## Reharmonization

Always state what is preserved (melody, bass notes on beats 1/3, cadence points,
rhythmic placement, tonal center). Then test the kept melody against **every**
new chord — each melody note should be a chord tone, an available tension, or a
clearly-resolving approach. Prefer smooth voice leading over chord-name density.
A reharm that adds five symbols but keeps every inner voice within a step of the
original is stronger than one that leaps around to look sophisticated.

## Chord-symbol policy (for the `//` comments)

Use conventional, readable symbols and keep them consistent through a piece:

```
Dm7   G7(b9)   Cmaj7(9)   F#m7b5   B7alt   Ebmaj7#11   A7sus
```

Distinguish `G7alt` / `G7(b9,#5)` / `G7(b13)` when the intended pitch collection
actually differs — because in wilios you will spell exactly those notes.

## Spelling changes as wilios pitch sets

wilios has no chord symbols. Convert each symbol to an explicit voicing (see
[`voicings.md`](voicings.md) for the voicing vocabulary), choosing inner voices
that move by step or common tone from the previous chord.

Worked example — `| Dm7 | G7alt | Cmaj7 |`, comped in the piano's middle
register, one chord per bar:

```wilios
// | Dm7 |        rootless: F A C E   (b3 5 b7 9)
<F3, A3, C4, E4> 1/1
// | G7alt |      3 b7 b9 b13 : B F Ab Eb   — F stays, A→Ab, C→B, E→Eb
<F3, Ab3, B3, Eb4> 1/1
// | Cmaj7 |      3 5 7 9 : E G B D          — F→E, Ab→G, B common, Eb→D
<E3, G3, B3, D4> 1/1
```

Every voice moves by a half step or holds — that is the point. Write the changes
as the comment, the notes as the chord, and let the roll (`dump --format roll`)
confirm the voice leading is as tight as you intended.

## Transposing a progression

`transpose(<chord>, semitones)` is the only chord transform. It works anywhere —
track scope, global scope, and inside a `func` body. To move a ii–V up a whole
tone:

```wilios
// at global scope, before any `track` (a `let` written after `track N`
// belongs to that track only, and tracks don't share variables):
let ii  = <F3, A3, C4, E4>
let v   = <F3, Ab3, B3, Eb4>
let ii2 = transpose(ii, 2)
let v2  = transpose(v, 2)

track 1
<ii> 1/1  <v> 1/1  <ii2> 1/1  <v2> 1/1
```

A `func` can also transpose its argument — `let lick = func(root) { <root> 1/8
<transpose(root, 4)> 1/8 <transpose(root, 7)> 1/8 }` — so one phrase replays in
any key (see `references/idiom-library.md`). Precompute-and-pass still keeps a
value reused across calls, and keeps a `rand`-derived value reproducible.

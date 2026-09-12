# Rhythm and feel

## How `swing` works here

`swing N` (N = 50–100, integer or float literal) applies to **8th-note pairs**:
the on-beat (even slot) 8th is lengthened, the off-beat (odd slot) 8th is
shortened, and the two always sum to the straight quarter. Quarter notes and
anything longer are **unaffected**; notes shorter than an 8th pass through
unchanged. Rests swing too, so the phase is preserved through a rest.

| `swing` | feel |
|---------|------|
| 50      | straight (default) |
| 58–62   | light / bright swing |
| 63–68   | medium jazz (`67` ≈ classic ⅔ + ⅓; at 120 BPM = 335 ms + 165 ms) |
| 70–78   | heavy / shuffle |
| 100     | max — off-beat collapses to 0 ms |

Set it **per track**, and use the same value on every track. The on/off slot
count resets at each bar start, anchored by `time_signature` — so the first 8th
of every bar is always the long slot. `time_signature` does nothing else: it
does **not** change note or rest lengths.

To write a straight-8ths bridge inside a swung tune, drop `swing 50` before it
and restore the swing value after.

## Comping rhythm cells

Comping is chords placed off the strong beats, with space. Write them as
rest/chord sequences; keep the voicing thin (see [`voicings.md`](voicings.md)).

```wilios
// "Charleston" — beat 1, and the 'and' of 2
<F3, A3, C4, E4> 1/8 rest 3/8   rest 1/8 <F3, A3, C4, E4> 1/8 rest 1/4
// push into the bar — 'and' of 4 anticipates the next chord
rest 1/2 rest 1/4 rest 1/8 <F3, Ab3, B3, Eb4> 1/8
// sparse: one stab per bar on the 'and' of 2
rest 3/8 <E3, G3, B3, D4> 1/8 rest 1/2
```

Each of those lines is exactly one 4/4 bar. Vary which cell you use bar to bar;
don't repeat one mechanically.

## Walking bass

Quarter notes, one per beat, mostly stepwise or by arpeggio, with each bar's
last note a **half-step or scale-step approach** to the next bar's root.

Construction per bar over chord X → next chord Y:

1. Beat 1: root of X (the strong anchor).
2. Beat 2: a chord tone — 3rd or 5th.
3. Beat 3: another chord tone, or a scale step continuing the line's direction.
4. Beat 4: approach tone into root of Y — chromatic (`Y root ± 1`) or the 5th
   above / 2nd below.

```wilios
// | F7            -> Bb7 |     F  A  C  (B = chromatic approach to Bb)
<F1> 1/4 <A1> 1/4 <C2> 1/4 <B1> 1/4
// | Bb7           -> F7 |      Bb D  F  (Gb -> F, or use C -> ... here Ab->G->Gb chromatic)
<Bb1> 1/4 <D2> 1/4 <F2> 1/4 <Gb2> 1/4
// | Dm7    G7 |   two chords in the bar: D F | G B
<D2> 1/4 <F1> 1/4 <G1> 1/4 <B1> 1/4
```

Keep the line inside roughly `E1`–`G2`. Model: `a_bass7` /`bridge_bass` in
`examples/bebop_trio.wilios`.

## Drum patterns

Presets and their recommended trigger pitches: `kick` → `<B1>`, `snare` →
`<A3>`, `hihat_c` (closed) / `hihat_o` (open) → `<F5>`, `ride` → `<F5>`,
`brushes` → `<A3>`. `ride` is a real cymbal wash (long shimmer) for the ride
pattern; `brushes` is a soft swish that stands in for `snare` on a ballad.

One-bar swing ride ("spang-a-lang"), as its own track:

```wilios
track 3
tempo 132
swing 66
volume 55
ride()
let r = 0
loop (r < 12) {
    r = r + 1
    <F5> 1/4  <F5> 1/8 <F5> 1/8  <F5> 1/4  <F5> 1/8 <F5> 1/8
}
```

Kick + snare on a second track (the preset is re-selected per note — fine for a
kit line, as `examples/bebop_trio.wilios` track 4 does):

```wilios
track 4
tempo 132
swing 66
volume 48
let k = 0
loop (k < 12) {
    k = k + 1
    kick() <B1> 1/4   snare() <A3> 1/4   kick() <B1> 1/4   snare() <A3> 1/4
}
```

The one-bar kit pattern can also be factored into a global `kit()` helper that
calls `kick()` / `snare()` and called from the loop — inlining as above is just
as valid and keeps the whole bar visible in one place.

## Odd meters and tuplets

`time_signature 7/8` etc. only anchors the swing phase and stamps metadata —
you build the meter yourself with durations that sum to the bar. State the
grouping in a comment (`7/8 = 2+2+3`). Tuplet durations are exact and never
drift, so triplets and quintuplets are safe:

```wilios
// eighth-note triplet (three in the space of a quarter)
<C4> 1/12 <D4> 1/12 <E4> 1/12
// quarter-note triplet
<C4> 1/6 <E4> 1/6 <G4> 1/6
// 5 in the space of a quarter
<C4> 1/20 <D4> 1/20 <E4> 1/20 <F4> 1/20 <G4> 1/20
```

`1/12` = a whole-note / 12 = an 8th-note triplet; `1/6` = a quarter triplet.
Variable durations (`n/d` with a variable) work but can't be dotted.

# Rhythm and feel

## How `swing` works here

`swing N` (N = 0–100, any numeric expression — literal, variable, or
arithmetic) applies to **8th-note pairs**: a note landing on the off-beat (odd
slot) 8th is **displaced later**, and its sounding length becomes the gap to the
next onset, so the pair plays long-short and still sums to the straight quarter.
Nothing else is touched — downbeats, quarter notes, tuplets, dotted values and
16ths all sound exactly as written under any feel, because swing never changes a
written position or duration.

To place a whole line behind or ahead of the beat — a soloist against a section
that stays on top — use `offset`, a per-track duration:

```wilios
offset 1/64      // behind the beat (~26ms at 144bpm; 1/48 is ~35ms)
offset -1/64     // ahead of it
offset 0         // back on it
let j = rand(-1, 1)
offset j/64      // humanize, note to note
```

Worked demo: `examples/feel.wilios`.

| `swing` | feel |
|---------|------|
| 50      | straight (default) |
| 52–58   | fast swing (above ~220 bpm the 8ths are nearly even) |
| 58–64   | medium-up / light swing |
| 64–70   | medium jazz (`67` ≈ triplet ⅔ + ⅓; at 120 BPM = 335 ms + 165 ms) |
| 70–78   | slow swing, heavy shuffle, 12/8 feel |
| 100     | max — off-beat collapses to 0 ms |

**Tempo sets the value more than style does.** On real recordings the swing
ratio falls as tempo rises: well above 2:1 at ballad tempos, about 2:1 only
around medium tempo, and close to straight at very fast tempos. A bebop line at
280 bpm with `swing 66` sounds stiff and corny; use ~54–58. Per-style, per-role
numbers: the tempo table in [`styles.md`](styles.md#how-to-set-swing).

Set it **per track**, and give the ride cymbal a bit more swing than the horn
and comp lines (e.g. ride `66`, lead `60` at 150 bpm) — on records the drummer's
ride is swung harder than the soloist's 8ths. Walking bass plays quarters, so
its value rarely matters. Different values never drift apart: swing only moves
off-beat onsets, downbeats stay shared. The on/off slot count resets at each bar
start, anchored by `time_signature` — so the first 8th of every bar is always
the long slot. `time_signature` does nothing else: it does **not** change note
or rest lengths.

To write a straight-8ths bridge inside a swung tune, drop `swing 50` before it
and restore the swing value after.

## Comping rhythm cells

Comping is chords placed off the strong beats, with space. Write them as
rest/chord sequences; keep the voicing thin (see [`voicings.md`](voicings.md)).

```wilios
// "Charleston" — beat 1, and the 'and' of 2
<F3, A3, C4, E4> 1/8 rest 1/4 <F3, A3, C4, E4> 1/8 rest 1/2
// push into the bar — 'and' of 4 anticipates the next chord
rest 1/2 rest 1/4 rest 1/8 <F3, Ab3, B3, Eb4> 1/8
// sparse: one stab per bar on the 'and' of 2
rest 3/8 <E3, G3, B3, D4> 1/8 rest 1/2
// reverse Charleston — 'and' of 1, then beat 3
rest 1/8 <F3, A3, C4, E4> 1/8 rest 1/4 <F3, A3, C4, E4> 1/8 rest 3/8
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
// | F7            -> Bb7 |     F  A  C, then B = chromatic approach from above
<F1> 1/4 <A1> 1/4 <C2> 1/4 <B1> 1/4
// | Bb7           -> F7 |      Bb Ab G, then Gb = chromatic approach from above
<Bb1> 1/4 <Ab1> 1/4 <G1> 1/4 <Gb1> 1/4
// | Dm7    G7 |   two chords in the bar: D A | G B  (B leads up to C)
<D2> 1/4 <A1> 1/4 <G1> 1/4 <B1> 1/4
```

Keep the line inside roughly `E1`–`D3`; going above `G2` for a bar or two
is normal and adds lift. Model: `a_bass7` /`bridge_bass` in
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

In swing the ride carries the time; the hi-hat (played with the foot) closes
on **2 and 4**; the kick is either silent or "feathered" very softly on all four
beats; the snare comps sparsely. Kick on 1 & 3 with snare on 2 & 4 is a rock
backbeat — do not use it for swing.

Hi-hat on 2 and 4, as its own track:

```wilios
track 4
tempo 132
swing 66
volume 45
hihat_c()
let k = 0
loop (k < 12) {
    k = k + 1
    rest 1/4 <F5> 1/4 rest 1/4 <F5> 1/4
}
```

Feathered kick: a separate track at very low `volume`, four quarters per bar
(for an occasional "bomb", raise `volume` before that hit and drop it back
after). Snare comping: a third track with a few
off-beat hits (e.g. the 'and' of 2, the 'and' of 4 leading into a new section),
varied bar to bar.

```wilios
track 5
tempo 132
swing 66
volume 18
kick()
let f = 0
loop (f < 12) {
    f = f + 1
    <B1> 1/4 <B1> 1/4 <B1> 1/4 <B1> 1/4
}
```

These parts can also be factored into global helpers and called from the loop
— inlining as above is just as valid and keeps the whole bar visible in one
place.

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

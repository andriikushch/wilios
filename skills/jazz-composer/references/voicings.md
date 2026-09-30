# Voicings

wilios has no voicing helper — every chord is a hand-spelled pitch set. This is
a vocabulary of shapes with concrete spellings and registers. For each change,
weigh: guide tones (3 & 7), common tones, semitone motion, register, tension
resolution.

Octave reference: `C4` = middle C. Comping piano sits roughly `C3`–`C5`; a lead
horn melody mostly `C4`–`G5` (trumpet reaches ~`C6`, tenor sax sounds down to
~`Ab2`); walking bass `E1`–`D3` (the double bass goes higher, but lines live
here).

## Shell voicings — root + 3 + 7

Sparsest usable jazz voicing; leaves room for a melody on top.

```wilios
// Cmaj7          C  E  B
<C3, E3, B3> 1/2
// Dm7            D  F  C
<D3, F3, C4> 1/2
// G7             G  F  B
<G2, F3, B3> 1/2
```

## Rootless (left-hand "A" / "B" shapes) — 3 5 7 9 or 7 9 3 5 (13 in place of 5 on dominants)

Bass covers the root; the comp plays four notes from `C3`–`A4`.

```wilios
// Dm7   A shape : F A C E      (b3 5 b7 9)
<F3, A3, C4, E4> 1/1
// G7    B shape : F A B E      (b7 9 3 13)
<F3, A3, B3, E4> 1/1
// Cmaj7 A shape : E G B D      (3 5 7 9)
<E3, G3, B3, D4> 1/1
```

Alternate A and B shapes chord to chord so the top voice moves by step, not by
leap.

## Drop-2 — take a close 4-note voicing, drop the 2nd voice from the top an octave

Wider, pianistic or guitaristic; good for melody harmonization.

```wilios
// Cmaj9 close  = E3 G3 B3 D4   ->  2nd from top is B3, drop it to B2:
<B2, E3, G3, D4> 1/1
// Am7  close   = C4 E4 G4 B4   ->  2nd from top is G4, drop it to G3:
<G3, C4, E4, B4> 1/1
```

Dropping the 3rd voice from the top instead gives **drop-3** (`<G2, E3, B3, D4>`
for the Cmaj9 above) — a guitar-friendly shape, but not drop-2.

## Upper-structure triad — a major triad over a dominant's 1-3-b7 to imply tensions

```wilios
// G13(#11)   (UST II)  : A major triad (A C# E) = 9 #11 13, over G F B
<G2, F3, B3, C#4, E4, A4> 1/2
// G7(#9,b13) (UST bVI) : Eb major triad (Eb G Bb) = b13 1 #9 — the "alt" sound
<G2, F3, B3, Eb4, G4, Bb4> 1/2
// G7(b9,#11) (UST bV)  : Db major triad (Db F Ab) = #11 b7 b9
<G2, F3, B3, Db4, F4, Ab4> 1/2
// C7(#9)     (UST bIII): Eb major triad (Eb G Bb) = #9 5 b7 — the mildest #9 colour
<C2, E3, Bb3, Eb4, G4, Bb4> 1/2
```

Check the triad against the guide tones: a triad that contains the natural 11
(e.g. Ab major = b9 11 b13 over G7) or the major 7 (e.g. D major over G7) fights
the 3rd or b7 and is not a usable upper structure.

## Quartal — stacked 4ths, for modal / McCoy-ish sounds

```wilios
// pure quartal (D dorian colour): E A D G C — three 4ths plus a 4th
<E3, A3, D4, G4, C5> 1/1
// "So What" voicing (Dm / D dorian): E A D G B — three 4ths + a major 3rd on top
<E3, A3, D4, G4, B4> 1/1
// the same shape moved diatonically down a step (the "So What" answer chord)
<D3, G3, C4, F4, A4> 1/1
// transposed for E dorian — keep the shape, not the white keys: F# B E A C#
<F#3, B3, E4, A4, C#5> 1/1
```

Start these around `E3`; below that the stacked 4ths get thick on the FM
presets.

## Spread / two-handed — root low, then 7 3 5 9 climbing

```wilios
// Cmaj9 spread : C  B  E  G  D
<C2, B2, E3, G3, D4> 1/1
// Fmaj9 spread : F  E  A  C  G
<F2, E3, A3, C4, G4> 1/1
```

## Rules of thumb

- Keep the guide tones (3 & 7) present in almost every voicing (quartal and sus
  voicings are the deliberate exception).
- Move the top voice by step where possible — check it on the piano roll.
- Below `C3`, keep intervals a 4th or wider (thirds get muddy on the FM presets).
- `transpose(<voicing>, n)` re-spells a shape to a new root cleanly, anywhere —
  track scope, global scope, or inside a `func` body (`func(root) { <root> 1/2
  <transpose(root, 7)> 1/2 }`).
- For guitar-style voicings, keep the span within a hand (≈ an octave and a
  fourth); wilios won't stop you writing an unplayable stretch, but it won't
  sound like a guitar either.

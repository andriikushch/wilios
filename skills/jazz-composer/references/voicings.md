# Voicings

wilios has no voicing helper — every chord is a hand-spelled pitch set. This is
a vocabulary of shapes with concrete spellings and registers. For each change,
weigh: guide tones (3 & 7), common tones, semitone motion, register, tension
resolution.

Octave reference: `C4` = middle C. Comping piano sits roughly `C3`–`C5`; a lead
horn `F4`–`C6`; bass `E1`–`G2`.

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

## Rootless (left-hand "A" / "B" shapes) — 3 5 7 9 or 7 9 3 5

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
// Cmaj7 close  = E G B D   ->  drop-2 = G  (E)  B D  ... spelled low-to-high:
<G2, E3, B3, D4> 1/1
// Am7  close   = C E G B   ->  drop-2:
<E2, C3, G3, B3> 1/1
```

## Upper-structure triad — a major/minor triad over a dominant to imply alterations

```wilios
// G7(#11)   : D major triad (D F# A) over G B F
<G2, B2, F3, D4, F#4, A4> 1/2
// G7(b9,b13): Ab major triad (Ab C Eb) over G B F
<G2, B2, F3, Ab4, C5, Eb5> 1/2
// C7(#9)    : Eb major triad over C E Bb
<C2, E3, Bb3, Eb4, G4, Bb4> 1/2
```

## Quartal — stacked 4ths, for modal / McCoy-ish sounds

```wilios
// "So What" voicing (Dm / D dorian): E A D G C
<E2, A2, D3, G3, C4> 1/1
// up a step for Em:
<F#2, B2, E3, A3, D4> 1/1
```

## Spread / two-handed — root low, then 7 3 5 9 climbing

```wilios
// Cmaj9 spread : C  B  E  G  D
<C2, B2, E3, G3, D4> 1/1
// Fmaj9 spread : F  E  A  C  G
<F2, E3, A3, C4, G4> 1/1
```

## Rules of thumb

- Keep the guide tones (3 & 7) present in almost every voicing.
- Move the top voice by step where possible — check it on the piano roll.
- Below `C3`, keep intervals a 4th or wider (thirds get muddy on the FM presets).
- `transpose(<voicing>, n)` re-spells a shape to a new root cleanly, anywhere —
  track scope, global scope, or inside a `func` body (`func(root) { <root> 1/2
  <transpose(root, 7)> 1/2 }`).
- For guitar-style voicings, keep the span within a hand (≈ an octave and a
  fourth); wilios won't stop you writing an unplayable stretch, but it won't
  sound like a guitar either.

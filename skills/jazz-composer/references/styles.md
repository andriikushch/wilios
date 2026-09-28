# Style packs

Per style: tempo range, `swing` value, preset choices, default form, and the
characteristic devices to lean on. Use these as starting points, not rules.
Translate the request into these musical characteristics rather than imitating a
named living musician.

## How to set `swing`

`swing` is the share of the beat taken by the first 8th note: `50` = straight,
`67` = triplet (2:1), `75` = 3:1. Two things matter more than style:

1. **Tempo.** Swing ratio falls roughly linearly as tempo rises. Ride-cymbal
   ratios measured on records go from about 3.5:1 (~78) at slow tempos down to
   nearly even (~50–55) around 300 bpm; 2:1 (67) only happens in a middle band
   of tempos.
2. **Instrument role.** Soloists swing _less_ than the drummer. At medium
   tempos the ride is above 2:1 while horn lines sit below it, with the horn
   landing slightly behind the ride on downbeats and together with it on
   upbeats.

So set swing per track: `ride` track higher, lead/comp tracks lower. Swing
only moves off-beat onsets, so tracks with different `swing` values still share
every downbeat and never drift (`tempo` and `time_signature` must still match).
To also put the horn behind the ride, add a small `offset 1/64`–`1/48` on the
lead track (see [`rhythm-and-feel.md`](rhythm-and-feel.md), demo
`examples/feel.wilios`).

"ride / drums" means the time-keeping track's role, not necessarily the `ride()`
preset: its long tail piles up into a buzz at spang-a-lang density, so for a
steady pattern keep its `volume` low or keep time on `hihat_c()` (as
`examples/bebop_trio.wilios` and `examples/blues_f.wilios` do) and save `ride()`
for sparser figures and accents.

| Tempo (bpm) | ride / drums | lead, comp, bass |
| ----------- | ------------ | ---------------- |
| 60–100      | 70–75        | 60–66            |
| 100–160     | 66–70        | 58–63            |
| 160–220     | 62–66        | 55–60            |
| 220–320     | 55–62        | 52–56            |

The per-style numbers below are read off this table for the style's typical
tempo range.

## Bebop

- **Tempo** 160–320 (medium-up to very fast; ballads exist but are not the
  signature) · **swing** ride 55–66, lead 52–60 (lower as tempo rises) ·
  **meter** 4/4
- **Presets** lead `brass` (often two horns in unison on the head); `upright`
  walking in 4; drums `ride` carries the time, `hihat_c` on 2 and 4, `kick`
  very soft with occasional off-beat accents ("bombs"), `snare` sparse
  comping; optional `comp_piano` with short, sparse stabs
- **Form** 32-bar AABA (especially rhythm changes), 12-bar blues, and
  contrafacts (new melody over a standard's changes); head – solos – (trading
  4s with drums) – head
- **Devices** harmonic rhythm of one or two chords per bar, ii–V chains and
  tritone substitutes, continuous 8th-note lines broken by triplet turns,
  bebop scales (an added chromatic passing tone so chord tones fall on the
  beat), chromatic approach tones, enclosures (upper + lower neighbor into a
  target), altered dominants (b9, #9, b13) resolving to the next chord,
  arpeggios reaching into the upper structure (9, 11, 13), phrases that start
  and end off the beat with accented upbeats. Model: `examples/bebop_trio.wilios`.
- **Library** `import "skills/jazz-composer/lib/bebop.wilios"` — `bebop_a_line`,
  `bebop_a_bass`, plus `licks.wilios` (`ii_v_i_maj`, `turnaround_maj`, `enclose`,
  `arp7`) and `grooves.wilios` (`swing_ride`, `kit_swing`). Demo:
  `examples/bebop_from_lib.wilios`. See `idiom-library.md`.

## Cool jazz

- **Tempo** 80–180 (mostly relaxed medium) · **swing** ride 62–72, lead 56–64 ·
  **meter** 4/4; the West Coast branch also experimented with 3/4 and odd
  meters (5/4, 9/8)
- **Presets** lead `brass` with soft attack and low `volume`; a second horn
  line in a lower register for counterpoint; `upright`; drums `brushes` or a
  light `ride`; piano optional (pianoless quartets were common)
- **Form** 32-bar AABA / ABAC standards and originals; arranged heads with
  written interludes and counterlines; shorter solos inside an arrangement
- **Devices** light, vibrato-poor tone and low dynamics, independent
  contrapuntal lines (two-voice counterpoint, canon, fugal entries), arranged
  mid-size ensemble textures with unusual colors (French horn, tuba), long
  linear melodic improvisation rather than virtuosic runs, relaxed rhythm
  section, classical influence in form and voice leading.

## Hard bop

- **Tempo** 60–260 (ballads to up-tempo; typical 120–220) · **swing** ride
  62–70, lead 56–62 · **meter** 4/4 (occasional 3/4)
- **Presets** two-horn front line on two tracks — `trumpet` on top, `brass`
  (tenor role) below — harmonized in 3rds, 4ths or unison; comp `comp_piano` (acoustic piano — the
  Rhodes belongs to late-60s soul jazz and fusion, not classic hard bop);
  `upright` walking; drums `ride` + `hihat_c` on 2 and 4 + active `snare`
  comping, fills and press rolls. A snare backbeat on 2 and 4 is only for
  soul-jazz / boogaloo tunes.
- **Form** 12-bar blues (major and minor), 16-bar tunes, 32-bar AABA; composed
  intros, vamps, shout choruses; Latin-A / swing-B hybrids
- **Devices** blues-scale inflections (b3, b5, b7 over major), gospel/church
  motion (`IV`–`iv`–`I` plagal, `bVII7`), minor-key tunes, call-and-response
  between horns and piano, harmonized two-horn heads, riff-based heads,
  pedal-point intros and vamps, hard-driving walking bass.

## Modal jazz

- **Tempo** 60–320 (medium swing is typical, but fast versions exist and
  some tunes use straight-8th grooves) · **swing** ride 60–68, lead 54–62, or
  `50` for straight-8th vamps · **meter** 4/4, 3/4 (often with a 6/8 feel)
- **Presets** lead `brass`/`strings`; comp `comp_piano` or `epiano` with
  **quartal** voicings (see [`voicings.md`](voicings.md)); `upright` pedal or
  walking; drums `ride` with triplet-based cross-rhythms
- **Form** long sections on one mode (8–16 bars). Classic model: 32-bar AABA
  with A = D Dorian and B = Eb Dorian (up a half step). Also 2-chord vamps and
  AABA forms built from sus4 chords.
- **Devices** "So What" voicing (three stacked 4ths + a major 3rd on top),
  quartal comping moving in parallel within the mode, sus4 chords, pedal
  points, pentatonic superimposition, side-slipping (a half step outside and
  back), wide intervallic leaps, motivic rather than chord-by-chord
  improvisation, intensity built from register, density and rhythm because
  the harmony is static.
- **Library** `import "skills/jazz-composer/lib/modal.wilios"` — `so_what_vamp`
  (quartal answer figure), `modal_pedal`, `dorian_frag`, plus `grooves.wilios`.
  Demo: `examples/modal_from_lib.wilios`. See `idiom-library.md`.

## Post-bop

- **Tempo** 60–300 · **swing** ride 60–68, lead 54–60, or `50` for
  straight-8th sections (many tunes switch between feels) · **meter** 4/4, 3/4,
  6/4; odd meters (5/4, 7/8) occasionally, more in later music
- **Presets** lead `brass`; comp `comp_piano` (mixed rootless + quartal);
  `upright` independent line; interactive drums (`ride`, `snare`, `kick`
  commenting on the soloist rather than keeping a fixed pattern)
- **Form** irregular section lengths (write odd bar counts out), extended or
  through-composed forms, reworked blues such as a 12-bar minor blues in 6/4
- **Devices** mixed functional / modal / non-functional harmony, constant
  structure (one chord quality moving by unusual intervals), sus and slash
  chords, upper-structure triads, smooth voice leading between distant chords,
  motif developed across sections, rhythmic displacement of a fixed cell,
  "time, no changes" solo sections, strong interplay between drums and
  soloist.

## Contemporary jazz

- **Tempo** any · **swing** one value on all tracks, 50–60 (often straight,
  8ths even) · **meter** odd
  and mixed, polymeter across tracks
- **Presets** full palette; `marimba` and `strings` for texture; `saw`/`square`
  `wave` layers; custom `fm { }` patches
- **Form** sectional, riff-plus-blowing, metric-modulation transitions
- **Devices** odd-meter grooves with a stated grouping (`9/8 = 2+2+2+3`),
  intervallic (4ths/5ths) melody writing, dense or very sparse orchestration,
  a repeated rhythmic ostinato under changing harmony, layered `loop`s at
  different lengths per track.

## Bossa nova

- **Tempo** 110–160, felt in 2 (Brazilian charts are usually in 2/4 or 2/2) ·
  **swing 50 (straight 8ths — do not swing)** · **meter** 4/4
- **Presets** lead `brass` (soft, breathy, low `volume`) or `epiano`; comp
  `comp_piano` or `epiano` imitating nylon-string guitar — short, rootless,
  gentle chords; `upright`; drums: cross-stick on `snare` at low `volume`
  playing the bossa clave, `hihat_c` soft steady 8ths, `kick` doubling the
  bass (surdo) pattern
- **Form** no single standard length: 32-bar AABA or ABAC is common, but many
  classics are irregular (e.g. an AABA with a 16-bar bridge = 40 bars, or
  longer through-composed forms). Take the length from the tune.
- **Devices**
  - **Bass (surdo)**: root on 1 (dotted quarter), 5th on the and-of-2 (short
    8th, can be ghosted), 5th on 3 (dotted quarter), and-of-4 anticipates the
    next bar's root. Beat 3 is the strong beat. Roots and 5ths only.
  - **Clave (3-2)**: two bars of 4/4 grouped 3+3+4+3+3 eighths — bar 1 on 1,
    and-of-2, 4; bar 2 on 2, and-of-3. It is similar to son clave; only the
    last stroke moves. Use it on cross-stick and let the comp line up with it.
    (Dotted-quarter/dotted-quarter/quarter = 3+3+2 is _one_ bar — the
    tresillo, i.e. only the three-side.)
  - **Comp**: syncopated guitar pattern derived from the samba tamborim part,
    chord changes anticipated on the and-of-4.
  - **Harmony**: `maj7`, `6/9`, `m7`, `m7b5` plus altered and colored
    dominants (`7b9`, `7#11`, `7b13`, `7#5`), diminished passing chords,
    chromatic descending inner voices, half-step key shifts.
  - **Melody**: smooth, often stepwise or repeated-note, syncopated
    anticipations, frequently resting on 9ths, 11ths and 13ths.
  - `pan` the comp slightly off-center.
- **Library** `import "skills/jazz-composer/lib/bossa.wilios"` — `bossa_bass_n`,
  `bossa_comp_n`, `bossa_melody_frag`, plus `grooves.wilios` (`kit_bossa`) and
  `comp.wilios` (`bossa_comp2`). Demo: `examples/bossa_from_lib.wilios`. See
  `idiom-library.md`.

```wilios
// bossa bass, one bar per chord (Fmaj7):
// root on 1, 5th on &2 and 3, root on &4 (anticipates next bar — on a chord
// change it is the new root, tied over; drop the next bar's 1 for that sound)
<F1> 3/8 <C2> 1/8 <C2> 3/8 <F1> 1/8
<F1> 3/8 <C2> 1/8 <C2> 3/8 <F1> 1/8

// bossa clave 3-2 on cross-stick (snare at <A3>), two bars: 3+3+4+3+3 eighths
<A3> 3/8 <A3> 3/8 <A3> 1/2 <A3> 3/8 <A3> 3/8
```

## Blues (jazz blues)

- **Tempo** 60–300 · **meter** 4/4 · **swing** by tempo:
  - slow blues / shuffle, 60–100: ride 70–75, lead 62–66 (12/8 feel)
  - medium, 110–180: ride 64–70, lead 58–63
  - up-tempo, 200–300: ride 56–64, lead 52–58
- **Presets** lead `brass`; comp `comp_piano`; `upright`; drums `ride` +
  `hihat_c` on 2 and 4; add a `snare` backbeat on 2 and 4 only for a shuffle
- **Form** 12-bar jazz blues:
  `| I7 | IV7 | I7 | v-7 I7 | IV7 | #IVdim7 | I7 | VI7(b9) | ii-7 | V7 | I7 VI7 | ii-7 V7 |`
  - Bird blues: `| Imaj7 | viiø7 III7 | vi-7 II7 | v-7 I7 | IV7 | iv-7 bVII7 | iii-7 VI7 | biii-7 bVI7 | ii-7 | V7 | I VI7 | ii-7 V7 |`
    (in F: `| Fmaj7 | Em7b5 A7 | Dm7 G7 | Cm7 F7 | Bb7 | Bbm7 Eb7 | Am7 D7 | Abm7 Db7 | Gm7 | C7 | F D7 | Gm7 C7 |`)
  - Minor blues: `| i-7 | i-7 | i-7 | i-7 | iv-7 | iv-7 | i-7 | i-7 | bVI7 | V7alt | i-7 | iiø7 V7alt |`
- **Devices** blue notes (b3, b5, b7 over the major chord), riff-and-answer
  head, `IV7` → `#IVdim7` → `I7/5` across bars 5–7, `v-7 I7` in bar 4 as a
  ii–V into IV, tritone-sub `bII7` in the turnaround, two-feel bass on the head
  and walking 4 for solos.
  Model: `examples/blues_f.wilios`.
- **Library** `import "skills/jazz-composer/lib/blues.wilios"` — `blues_walk12`
  (a full 12-bar walking chorus), `blues_riff4` / `blues_riff_head`, plus
  `grooves.wilios`. Demo: `examples/blues_from_lib.wilios`. See
  `idiom-library.md`.

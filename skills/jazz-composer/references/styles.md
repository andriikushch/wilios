# Style packs

Per style: tempo range, `swing` value, preset choices, default form, and the
characteristic devices to lean on. Use these as starting points, not rules.
Translate the request into these musical characteristics rather than imitating a
named living musician.

## Bebop

- **Tempo** 180–300 · **swing** 60–66 · **meter** 4/4
- **Presets** lead `brass`; `bass` (or `upright`); drums `ride` + `kick`/`snare`
- **Form** 32-bar AABA, rhythm changes, or 12-bar blues; head–solos–head
- **Devices** fast harmonic motion (two chords/bar), continuous 8th-note lines,
  chromatic approach tones, enclosures (upper + lower neighbor into a target),
  altered dominants, arpeggios through the 7th and 9th, phrases that start and
  end off the beat. Model: `examples/bebop_trio.wilios`.
- **Library** `import "skills/jazz-composer/lib/bebop.wilios"` — `bebop_a_line`,
  `bebop_a_bass`, plus `licks.wilios` (`ii_v_i_maj`, `turnaround_maj`, `enclose`,
  `arp7`) and `grooves.wilios` (`swing_ride`, `kit_swing`). Demo:
  `examples/bebop_from_lib.wilios`. See `idiom-library.md`.

## Cool jazz

- **Tempo** 100–160 · **swing** 56–62 · **meter** 4/4 (sometimes 3/4)
- **Presets** lead `brass` or `epiano`; `bass` or `upright`; brushed feel —
  `brushes` (or `hihat_c` light), low `volume`
- **Form** 32-bar AABA, ABAC; often just head + one solo chorus
- **Devices** space and rests, lighter articulation (longer note values),
  transparent 2–3 voice textures, restrained dynamics via register not volume,
  melodic (not scalar) development, counterlines between lead and bass.

## Hard bop

- **Tempo** 130–210 · **swing** 63–70 · **meter** 4/4
- **Presets** lead `brass`; comp `epiano`; `bass`; `kick`/`snare`/`hihat_c`
  with a strong backbeat on 2 and 4
- **Form** 12-/16-bar blues, 32-bar AABA
- **Devices** blues scale inflections (b3, b5, b7 over major), gospel/church
  `IV`–`iv`–`I` and `bVII` moves, call-and-response between lead and comp,
  hard-driving quarter-note bass, riff-based heads.

## Modal jazz

- **Tempo** 80–160 · **swing** 55–62 (or straight) · **meter** 4/4, 3/4, 6/8
- **Presets** lead `brass`/`strings`; comp `epiano` with **quartal** voicings
  (see [`voicings.md`](voicings.md)); `bass` pedal or slow walk
- **Form** long sections on one chord (8–16 bars), 2-chord vamps, AABA where
  each letter is a mode
- **Devices** sustained centers, pentatonic and modal melody, wide intervallic
  leaps, motivic (not vertical) improvisation, motion from register / rhythm /
  orchestration since the harmony is static.
- **Library** `import "skills/jazz-composer/lib/modal.wilios"` — `so_what_vamp`
  (quartal answer figure), `modal_pedal`, `dorian_frag`, plus `grooves.wilios`.
  Demo: `examples/modal_from_lib.wilios`. See `idiom-library.md`.

## Post-bop

- **Tempo** 120–260 · **swing** 58–66 · **meter** 4/4, 5/4, 7/8, mixed
- **Presets** lead `brass`; comp `epiano` (mixed rootless + quartal); `bass`
  independent line; interactive drums
- **Form** through-composed or irregular AABA (10-bar A, 6-bar B, etc.) — write
  the odd bar counts out
- **Devices** hybrid functional/modal harmony, asymmetric phrasing, slash chords
  and upper structures, sophisticated stepwise voice leading, motif developed
  across sections, rhythmic displacement of a fixed cell.

## Contemporary jazz

- **Tempo** any · **swing** 50–60 (often straight, 8ths even) · **meter** odd
  and mixed, polymeter across tracks
- **Presets** full palette; `marimba` and `strings` for texture; `saw`/`square`
  `wave` layers; custom `fm { }` patches
- **Form** sectional, riff-plus-blowing, metric-modulation transitions
- **Devices** odd-meter grooves with a stated grouping (`9/8 = 2+2+2+3`),
  intervallic (4ths/5ths) melody writing, dense or very sparse orchestration,
  a repeated rhythmic ostinato under changing harmony, layered `loop`s at
  different lengths per track.

## Bossa nova

- **Tempo** 120–170 · **swing 50 (straight 8ths — do not swing)** · **meter** 4/4
- **Presets** lead `brass`/`epiano`; comp `comp_piano` or `epiano` (rootless,
  gentle); `upright` playing the two-note "1 and 3-and" pattern; `brushes` soft
- **Form** 32-bar AABA or 16-bar
- **Devices** the clave-derived comp rhythm (dotted-quarter / dotted-quarter /
  quarter across two bars), bass on root–5th only, smooth `maj7`/`m7`/`m7b5`
  harmony, legato singable melody, `pan` the comp slightly off-center.
- **Library** `import "skills/jazz-composer/lib/bossa.wilios"` — `bossa_bass_n`,
  `bossa_comp_n`, `bossa_melody_frag`, plus `grooves.wilios` (`kit_bossa`) and
  `comp.wilios` (`bossa_comp2`). Demo: `examples/bossa_from_lib.wilios`. See
  `idiom-library.md`.

```wilios
// bossa bass cell, two bars over one chord (root then 5th)
<F1> 1/4 rest 1/4 <C2> 1/4 rest 1/4
<F1> 1/4 rest 1/4 <C2> 1/4 rest 1/4
```

## Blues (jazz blues)

- **Tempo** 60–180 · **swing** 63–72 (shuffle end) · **meter** 4/4 (12/8 feel)
- **Presets** lead `brass`; comp `epiano`; `bass`; `kick`/`snare`/`hihat_c`
- **Form** 12-bar: `| I7 | IV7 | I7 | I7 | IV7 | IV7 | I7 | VI7 | ii7 | V7 |
  I7 VI7 | ii7 V7 |` (bird blues adds ii–Vs through bars 1–4)
- **Devices** blue notes (b3, b5), riff-and-answer head, `IV7`→`#IVdim`→`I/5`
  in bar 6, tritone-sub `bII7` in the turnaround, walking or two-feel bass.
  Model: `examples/blues_f.wilios`.
- **Library** `import "skills/jazz-composer/lib/blues.wilios"` — `blues_walk12`
  (a full 12-bar walking chorus), `blues_riff4` / `blues_riff_head`, plus
  `grooves.wilios`. Demo: `examples/blues_from_lib.wilios`. See
  `idiom-library.md`.

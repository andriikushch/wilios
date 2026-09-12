# Idiom library (importable `.wilios`)

The idioms in `harmony.md` / `voicings.md` / `rhythm-and-feel.md` are also
shipped as **callable `func`s** under `skills/jazz-composer/lib/`. Import one
file and call its helpers from a track; a phrase is now built from sub-phrases,
so these compose.

## How to use them

```wilios
import "<relative>/skills/jazz-composer/lib/bebop.wilios"

track 1
tempo 200
swing 63
volume 90
brass()
bebop_a_line(Bb4)          // an 8-bar line, built from ii-V-I + turnaround licks
```

- The import path is **relative to your file** and must stay inside the repo
  (the sandbox root). From `examples/` that is
  `../skills/jazz-composer/lib/<file>.wilios`. Count the `../` from wherever
  your file actually lives.
- Every helper is **called in statement position** (on its own line) and emits
  notes onto the current track. Nothing is returned — a statement call discards
  return values, and a `let`/param bound inside a body is gone when the call
  returns.
- A helper does **not** set `tempo` / `swing` / `time_signature` — those are
  per-track and literal-only. Set them yourself at the top of every track.
- Melodic helpers take a **root pitch** and place themselves with `transpose`,
  so one lick plays in any key. Comp helpers take a **hand-spelled voicing**
  (a chord value). Drum/bass helpers take a **bar count** and/or pitches.
- One flat namespace: a track-local `let` that shares a helper's name shadows
  it for that track.

## Files

### `lib/theory.wilios` — scales, chord-tone sets, array helpers
BUILDERS return a pitch array (call in expression position); PLAYERS emit one
(call on their own line). Everything is placed from the root with `transpose`,
so a flat root gives flat spellings.

| helper | gives |
|---|---|
| `scale_major/dorian/mixo/lydian/mel_minor/altered/blues(r)` | the scale as a 6–7 note array from root `r` |
| `scale_bebop_dom(r)` / `scale_bebop_maj(r)` | the 8-note bebop scales |
| `chord_maj7/maj6/dom7/min7/min7b5/dim7/dom7b9(r)` | 4 chord tones as an array from root `r` |
| `nth(a, i)` / `wrap(a, i)` | element `i` (exact / modulo length, for ostinati) |
| `run_seq(arr, dn, dd)` | play `arr` in order, each note `dn/dd` long |
| `transpose_seq(arr, n, dn, dd)` | play `arr` shifted `n` semitones |
| `arp_seq(arr, dn, dd)` | ascend then descend `arr` (up/down arpeggio over any chord set) |

### `lib/licks.wilios` — melodic bebop idioms
| helper | plays |
|---|---|
| `enclose(t)` | chromatic upper+lower neighbour into `t`, as an 8th-note triplet |
| `approach_below(t)` / `approach_above(t)` | one chromatic approach note + `t`, two 8ths |
| `arp7(root)` / `arp_maj7(root)` / `arp_min7(root)` | arpeggio through the 7th, four 8ths |
| `ii_v_i_maj(key)` | a 2-bar bebop line over `\| iim7 V7 \| Imaj7 \|` (built from the above) |
| `turnaround_maj(key)` | a 2-bar line over `\| I VI7 \| ii V7 \|` |

### `lib/comp.wilios` — comping rhythm cells (one 4/4 bar each, voicing `v`)
`charleston(v)` · `push(v)` (anticipation into the next bar) · `stab(v)` (one
'and'-of-2 hit) · `bossa_comp2(v)` (two-bar clave figure, straight).

### `lib/grooves.wilios` — drums + bass (imports the FM presets)
`swing_ride(bars)` · `kit_swing(bars)` · `kit_bossa(bars)` · `kit_shuffle(bars)`
· `walk_bar(n1, n2, n3, n4)` (one walking-bass bar from four pitches) ·
`two_feel(root, fifth, bars)` · `bossa_bass2(root, fifth)` (two-bar cell).

### Style packs — `lib/{bebop,bossa,blues,modal}.wilios`
Each opens with a `//` header block of that style's tempo range / `swing` /
presets / form / devices (copy the settings into every track), re-imports the
idiom files above, and adds signature helpers:

| pack | helpers |
|---|---|
| `bebop.wilios` | `bebop_a_line(key)` (8-bar A line), `bebop_a_bass(key, cycles)` (walking I-VI-ii-V) |
| `blues.wilios` | `blues_walk12(key)` (12-bar walking bass), `blues_riff4(key)` / `blues_riff_head(key)` |
| `bossa.wilios` | `bossa_bass_n(root, fifth, n)`, `bossa_comp_n(v, n)`, `bossa_melody_frag(key)` |
| `modal.wilios` | `so_what_vamp(bars)` (quartal answer figure), `modal_pedal(root, bars)`, `dorian_frag(key)` |

## Worked demos

`examples/{bebop,blues,bossa,modal}_from_lib.wilios` — each is a short,
through-composed piece assembled entirely from these imports. They are wired
into `make smoke`. Compare `examples/blues_f.wilios` / `examples/bebop_trio.wilios`,
which spell the same material by hand.

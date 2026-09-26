---
name: jazz-composer
description: >
  This skill should be used when the user asks to compose, arrange, reharmonize,
  voice, or analyze jazz as a wilios .wilios file — for example "write a jazz
  tune", "compose a bebop head", "a ii-V-I etude in wilios", "reharmonize these
  changes", "give me a walking bass line", "make a bossa nova", "swing blues for
  trio". Produces validated, playable multi-track .wilios source that applies
  real jazz craft (form, harmony, motif, voice leading, feel) within the DSL's
  constraints, and drives the validate -> dump feedback loop, with WAV render
  and MIDI export as optional outputs.
version: 1.0.0
---

# Jazz Composer

Turn a musical intention into a **coherent, playable `.wilios` file**. Compose
music that sounds intentional, not merely "jazzy". Harmonic complexity is not
musical sophistication — a strong tune can be harmonically simple.

Work the musical decisions in this order, and let each one reinforce the next
(tension/release, motif, phrase direction, harmonic destination, orchestration,
contrast):

> musical identity → form → rhythm/groove → harmony → melody → voice leading →
> arrangement → improvisational framework

The full method — musical brief, bar-counted form, harmonic architecture, motif
development, melody construction, self-review checklist — is in
[`references/workflow.md`](references/workflow.md). Read it before composing
anything longer than a single phrase.

## Modes

Infer which one the request wants; they share the workflow and output contract.

- **compose** — an original piece. Full workflow, ending in a written `.wilios`.
- **arrange** — score existing material for a given band. Preserve the user's
  melody and form unless told otherwise; assign roles per track.
- **reharmonize** — new changes under a kept melody. **State what is preserved**
  (melody, bass, cadence points, rhythmic placement, tonal center) and test the
  melody against every new chord. See [`references/harmony.md`](references/harmony.md).
- **voicings** — practical voicings for a named instrument/register, spelled as
  wilios pitch sets. See [`references/voicings.md`](references/voicings.md).
- **analyze** — describe supplied music. Separate objective observation from
  interpretation. Do not invent notes or chords that were not supplied.

## Targeting wilios — the constraints that change how you compose

wilios is a real synth/sequencer DSL, not notation. These limits are load-bearing;
the exhaustive list with workarounds is in
[`references/wilios-gotchas.md`](references/wilios-gotchas.md).

- **Chords are explicit pitch sets, nothing else.** No chord symbols, no
  `chord()`, no roman numerals, no scale/degree notation. Write the changes as
  `//` comments above each phrase and spell every voiced note:
  `<Bb2, D3, Ab3, C4> 1/2` for a Bb7 shell + color. Voicing choices live in
  [`references/voicings.md`](references/voicings.md).

- **A `func` body resolves names at the call site, not the definition site.** It
  can call other top-level `func`s, presets, and the builtins (`transpose`,
  `rand`, `len`, `print`), and `func`s may recurse. Keep helper and phrase
  `func`s at **global scope** — a track-local `let` with the same name as a
  global `func` shadows it for anything called from that track. Anything a body
  binds (a parameter, or a `let`) is discarded when the call returns, so
  precompute-and-pass is still the move when you want a value reused or `rand`
  kept reproducible.

  ```wilios
  // all fine now
  let lick  = func(n) { <transpose(n, 3)> 1/8 }
  let phrase = func(root) { lick(root) lick(transpose(root, 5)) }
  // still preferred for reproducibility: fix the value once, pass it in
  let up3 = transpose(C4, 3)
  let fixed = func(n) { <n> 1/8 }
  ```

- **`tempo` is a literal int, and tempo/feel are per-track.** Every track must
  repeat the **same** `tempo`, `swing`, and `time_signature`, or the tracks
  drift apart. The repo-root examples `examples/blues_f.wilios` and
  `examples/bebop_trio.wilios` both restate `tempo` + `swing` in every
  `track` block.

- **Swing is an 8th-note-pair feel.** `swing 60`–`72` ≈ medium jazz; range is
  50 (straight) to 100. It lengthens the on-beat 8th and shortens the off-beat
  8th; quarter notes and longer are untouched, notes shorter than an 8th pass
  through: `swing` displaces the off-beat rather than rewriting durations, so
  tuplets and dotted values stay exact under any feel. Set it per track —
  `swing` takes an expression, so `let feel = 63` at global scope and
  `swing feel` in each track keeps one source of truth. To put a line *behind*
  the beat, use `offset 1/64` (per track, a duration; `offset 0` returns).
  `time_signature` only anchors the bar phase and
  is metadata — it does **not** change note lengths. Details:
  [`references/rhythm-and-feel.md`](references/rhythm-and-feel.md).

- **No per-note dynamics.** Velocity equals the track `volume`. Get contrast
  from register, rhythm, note density, rests, `pan`, and preset choice.

- **Presets** come from `import`ing the FM library (path is relative to the
  `.wilios` file — `../lib/lib.wilios` from the repo `examples/` dir). 13 of
  them: tonal `brass`, `epiano`, `bass`, `upright`, `marimba`, `strings`,
  `comp_piano`; drums `kick`, `snare`, `hihat_c`, `hihat_o`, `ride`, `brushes`.
  `upright` is a rounder acoustic bass, `comp_piano` sustains (unlike the
  percussive `epiano`), `ride` is a real cymbal wash distinct from `hihat_c`,
  `brushes` is a soft swish. For anything else, define a custom
  `let name = func() { wave ...; fm { ... } }` at global scope.

- **Control flow is thin.** No `break` / `continue` (design the loop count),
  no `else if` (nest `if`/`else`), no unary `!` (write `x == false`) — but the
  binary `!=` comparison works (`loop (i != N)`, `if (x != y)`). A bounded
  `loop (i < N) { i = i + 1  ... }` is fine; `loop (true)` forces `--duration`
  on every tool below.

## The feedback loop

After writing the file, always run it. From the repo root:

```bash
cargo run -- dump FILE.wilios --format roll     # compiles it; prints an ASCII
                                                # piano roll to eyeball rhythm
                                                # and voice leading, no audio
```

`dump` runs the interpreter, so a parse or runtime error surfaces there. Read the
roll: are the bar counts right, does the bass land on every beat, is the melody
phrased against the harmony, do the tracks line up? Revise and re-`dump`. That is
the core loop.

Optional outputs, once the roll looks right:

```bash
cargo run -- render FILE.wilios -o /tmp/out.wav # offline WAV — to hear it
cargo run -- midi   FILE.wilios -o /tmp/out.mid # Standard MIDI File — for a DAW
```

Produce the WAV when you (or the user) want to hear the result; produce the MIDI
when the user wants a DAW file. Neither is needed to iterate.

If the **wilios MCP server** is connected, prefer it for the static checks:
`validate` (lex + parse + resolve, no execution — returns `ok: true/false` with
spans and "did you mean" hints), `describe_symbol` / `search_stdlib` (confirm a
preset or builtin exists and its signature **before** using it), `dump_events`
(the timeline; pass `format: "roll"` for the same piano roll).

Iterate: on a revision, change only the dimension asked for ("darker" → harmony,
register, voicing, orchestration; "simpler harmony" → fewer substitutions and
alterations) and keep form, groove, and identity.

## Output contract

Deliver **one validated `.wilios` file**, shaped like `examples/blues_f.wilios`
(repo root):

1. A header `//` comment block: title, style, tempo, feel, meter, form, the
   chord changes (bar by bar), and the three commands to run it.
2. `import "<relative>/lib/lib.wilios"` for the FM presets. For a named style,
   also `import "<relative>/skills/jazz-composer/lib/<style>.wilios"` — the pack
   re-exports the idiom helpers (licks, comping cells, grooves, walking bass)
   and pulls the presets in transitively. See
   [`references/idiom-library.md`](references/idiom-library.md).
3. Phrase `func`s grouped by voice (lead, bass, comp, drums), each preceded by a
   `//` comment naming the bars and the changes it covers.
4. One `track` block per voice, each opening with the **matched**
   `tempo` / `swing` / `time_signature`, then `volume` / `pan`, then the preset
   call, then the phrase calls (or a bounded `loop`).

Put the music first. State any assumption you made (key, tempo, band, form) in
one line, don't over-explain. When the user asks for a complete piece, write it —
don't just describe how one might.

## References

- [`references/workflow.md`](references/workflow.md) — the full compose method + self-review checklist.
- [`references/harmony.md`](references/harmony.md) — functional / modal / chromatic harmony, ii-V-I family, substitutions, reharmonization; spelling changes as pitch sets.
- [`references/voicings.md`](references/voicings.md) — rootless / shell / drop-2 / upper-structure voicings as concrete wilios pitch sets, with registers.
- [`references/rhythm-and-feel.md`](references/rhythm-and-feel.md) — how `swing` behaves here, comping cells, walking-bass construction, drum patterns, odd meters and tuplets.
- [`references/styles.md`](references/styles.md) — style packs (bebop, cool, hard bop, modal, post-bop, contemporary, bossa nova, blues): tempo, `swing`, presets, form, characteristic devices.
- [`references/idiom-library.md`](references/idiom-library.md) — the idioms as importable `.wilios` helpers (`skills/jazz-composer/lib/`): licks, comping cells, grooves, walking bass, and the four style packs.
- [`references/wilios-gotchas.md`](references/wilios-gotchas.md) — every DSL limitation that bites a composer, with the workaround.
- `examples/blues_f.wilios` (repo root) — minimal annotated 12-bar F blues, trio; walk the whole loop with it.
- `examples/bebop_trio.wilios` (repo root) — advanced: 32-bar AABA rhythm changes, hand-spelled, the canonical multi-track model.
- `examples/{bebop,blues,bossa,modal}_from_lib.wilios` (repo root) — short pieces assembled entirely from the idiom library, one per style pack.

# Music-authoring ideas & tasks

Backlog for making composition in wilios **easier** (less hand-typing, faster
iteration) and **more creative** (higher-level musical building blocks).

Most items came out of writing `examples/bebop_trio.wilios` (a 32-bar bebop trio) —
the friction points are noted per item.

Priority: **P0** blocking / highest leverage · **P1** high value · **P2** nice to have.
Effort: **S** < 1 day · **M** a few days · **L** 1–2 weeks+.

---

## 1. Feedback loop — hear / see the result

> Friction: the CLI opens an audio device immediately and panics with no device
> (`DeviceNotAvailable`). There is no other output. The whole trio was composed blind
> and only checked with `mcp__wilios__validate` + a throwaway scheduling test.

- [x] **P0 / M — Offline render to WAV.** `wilios render FILE -o out.wav [--duration S] [--sample-rate]`.
      No `cpal` device; runs the interpreter + `wilios-synth::Mixer` straight to a WAV via
      `wilios_cli::render` (atomic `.partial` write, renamed into place only on success;
      `MAX_RENDER_SECS` 600 s safety cap). The "lex → parse → interpret" core is now
      `wilios_cli::pipeline`, shared by `play`, `render`, and `dump`; the audio callback is
      just one consumer of it.

  > **Endless loops.** A `.wilios` program can run forever: `loop (true) { ... }`, or any
  > loop whose condition never goes false. `Interpreter::all_tracks_finished()` then never
  > returns `true` and each `schedule_until` call keeps emitting events. So `wilios render`
  > without `--duration` can only render up to a safety cap (`MAX_RENDER_SECS`, 600 s) and
  > then **hard-errors with no output file** — looping pieces must pass `--duration`.
  > (A note shorter than its attack also never releases, with the same practical effect.)
  > `dump` already handles this via its `max_ms` budget; MIDI export and the MCP `render`
  > tool below need the same bound.
- [x] **P0 / S — Event dump.** `wilios dump FILE [--format json|text] [--duration S]` → per-track
      note timeline (`at_ms`, `at_beats`, `pitch`, `freq_hz`, `dur`, `velocity`, `pan`, `waveform`,
      ADSR, full FM config). Same endless-loop time bound as `render`. Shared driver
      `Interpreter::schedule_to_end` + serde DTOs in `wilios_core::dump`; also exposed as the
      `dump_events` MCP tool.
- [x] **P1 / S — ASCII piano-roll / per-bar text score** from the event dump, for quick
      eyeballing of rhythm and voice-leading without listening. Shipped as
      `wilios dump FILE --format roll` and the `dump_events` MCP tool's `format: "roll"`
      option (`wilios_core::dump::render_piano_roll`): one 16th-note grid per track,
      `#` onset / `=` sustain / `.` empty, `|` bar lines, rows spanning every chromatic
      step the track uses. `NoteDump` also gained a `midi_note` field.
- [x] **P1 / M — MIDI export.** Shipped as `wilios midi FILE -o OUT.mid [--duration S]`
      (a dedicated subcommand, not a `dump` format — binary output needs `-o` and an
      atomic `.partial` write, like `wilios render`). Standard MIDI File format 1, 480
      PPQ, musical ticks straight from each note's exact `at_beats`; every `tempo`
      change becomes a `Tempo` meta event. wilios tempo is per-track and a MIDI file
      has one global tempo map, so the lowest-numbered track's tempo history is used
      and a warning is logged if another track disagrees. MIDI is **not** exposed over
      MCP (binary artifact — belongs with the future MCP `render` tool).
- [ ] **P2 / M — MCP `render` tool.** Runs a source (inline `source` xor `path`) and returns,
      in one call: the rendered **WAV** (a `wilios://render/<id>.wav` resource link by default,
      inline base64 `AudioContent` when small enough), a **waveform PNG** and a
      **log-frequency spectrogram PNG** — the artifacts an agent can actually read: is a track
      silent? is the FM patch bright or dull? is there aliasing (energy folding down from
      Nyquist)? — and a small **analysis** block (peak / RMS dBFS, clipped-sample count,
      limiter-engaged ratio, leading/trailing silence, per-track note count + pitch range).
      `want: [...]` selects a subset so an agent can ask for just the spectrogram + analysis
      and skip the big audio payload. Fills the last gap in the feedback loop: `validate`
      catches compile errors, `dump_events` shows the note grid, but neither tells you what
      the piece *sounds* like.

  > **Mirrors `dump_events` exactly** (it is the second executing tool): `source` xor `path`,
  > sandbox via `resolve_import_path`, 1 MB source cap, `spawn_blocking` + wall-clock
  > timeout, an `max_ms` composition-time budget clamped to `[1000, 600000]`. A
  > non-terminating piece (endless loop, or a note shorter than its attack) comes back as a
  > *successful* result with `finished: false` and audio/images truncated at the bound — not
  > a tool error. `isError` stays reserved for bad args, an unreadable path, a sandbox
  > escape, or the timeout.
  >
  > **Prerequisite refactor.** The device-independent render path (`wilios_cli::pipeline`,
  > `::voices`, `::render`) lives in `wilios-cli`, which links `cpal`. Lift it into a shared
  > `wilios-render` crate (deps: `wilios-core` with `serde`, `wilios-synth`, `hound`, plus
  > new `rustfft` + `png` for the images — all pure-Rust, no system libs) so `wilios-mcp`
  > never gains an audio-device dependency; extend the CI `cargo tree | grep cpal` guard to
  > `wilios-render` and `wilios-mcp`. `wilios-cli`'s `render`/`play` then consume that crate.
  >
  > **Not reproducible yet.** A piece calling `rand()` renders differently on every call
  > until seeded randomness (§4) lands; the result sets `used_rng: true` rather than
  > implying determinism. Two renders of an RNG-free piece are byte-identical.
  >
  > Test by invariants, not golden images: a 440 Hz note peaks in the 430–450 Hz spectrogram
  > bin; `rest 1/1` is an all-floor spectrogram with `peak_dbfs ≈ -inf`; a deliberately hot
  > mix reports `clipped_samples > 0`; PNG bytes decode to the requested dimensions.
- [x] **P2 / S — `--headless` / CI smoke mode** that just asserts "schedules with no
      runtime error, all tracks reach the same end time" (what the temp test did by hand).
      Shipped as `wilios smoke FILE [--duration S]` (`wilios_cli::smoke`): (a) `schedule_to_end`
      returns `Ok` and `finished`, (b) every track's exact `nominal_position` is equal (trailing
      rests count; skipped for a `--duration`-bounded run). Wired into CI + `make smoke` over
      `examples/bebop_trio.wilios`.

## 2. Language: composition of phrases

- [x] **P0 / M — Allow `func` to call other top-level `func`s** (and mutual recursion).
      Building phrases out of sub-phrases is the core compositional move. Shipped: a
      `func` body now runs against the call-site environment plus its parameters, so it
      can call other functions, presets, and the built-ins, and functions may recurse or
      be mutually recursive (`Stmt::Call`/`Expr::Call` clone the env instead of replacing
      it; expression-position recursion is depth-capped). Scoping is call-site (dynamic),
      not lexical — see `doc/language-reference.md` §11 / Known Limitations.
- [ ] **P1 / M — `section NAME { ... }` + `form [A A B A]`** so arrangement is declared,
      not hand-concatenated (`a7() end_turn() a7() end_bridge() bridge() ...`).
- [ ] **P2 / S — First/second endings, repeats, `D.C. al coda`.**

## 3. Language: musical primitives (less hand-typed pitch spelling)

> Friction: ~600 notes entered by hand, each as an absolute pitch token.

- [ ] **P1 / M — Scales & chords as first-class values.** `scale(Bb, bebop_dominant)`,
      `chord(G7)` → pitch arrays; index/slice/iterate them.
- [ ] **P1 / M — Degree notation** relative to a current key/chord: `[1 3 5 b7]`, so one
      lick can be replayed over any chord.
- [ ] **P1 / M — Transform ops** beyond `transpose`: `invert`, `retrograde`,
      `arpeggiate(chord, pattern)`, `enclose(target)` (chromatic approach / enclosure),
      `walk(from, to, beats)` (walking-bass connector).
- [ ] **P2 / L — `changes { 1: Bbmaj7 G7  2: Cm7 F7  ... }` block** — declare harmony once;
      melody/bass/drum functions query "what chord is sounding now". Enables
      chord-aware bass lines and comping that follows the head.

## 4. Language: expression & feel

> Friction: everything is quantized and same-velocity; `swing` is the only feel control.
> Per-note overrides are already listed as pending in `CLAUDE.md`.

- [ ] **P1 / M — Per-note velocity / accent / ghost / articulation.**
- [ ] **P1 / M — `humanize`** (timing + velocity jitter), per-track laid-back / on-top feel.
- [ ] **P1 / S — Seeded, constrained randomness:** `seed 42`, `rand_note(scale)`,
      weighted choice. Raw `rand(min, max)` ints are not musical, and runs aren't
      reproducible.
- [ ] **P2 / S — `tempo` as an expression** (currently integer-literal only) + `accel` / `rit`.
- [ ] **P2 / M — Cross-track awareness** (drums catch the melody's accents; call-and-response).

## 5. Standard library (`.wilios` only — mostly no engine change)

- [x] **P1 / S — scale / chord-tone / array helper library.** Shipped as
      `skills/jazz-composer/lib/theory.wilios` — `scale_*` / `chord_*` builders
      that return root-relative pitch arrays, `nth` / `wrap` accessors, and
      `run_seq` / `transpose_seq` / `arp_seq` players. Under the skill dir, not
      `lib/`, so it travels with `make install-skills`; `modal.wilios` consumes
      it. Enclosure and fixed root/3/5/7 arpeggios stay in `licks.wilios`.
- [x] **P1 / M — bebop lick / turnaround / walking-bass library.** Shipped as
      `skills/jazz-composer/lib/licks.wilios` (`enclose`, `arp7`/`arp_maj7`/`arp_min7`,
      `ii_v_i_maj`, `turnaround_maj`) + `skills/jazz-composer/lib/bebop.wilios`
      (`bebop_a_line`, `bebop_a_bass`). Under the skill dir, not `lib/`, so it
      travels with `make install-skills`.
- [x] **P1 / M — parameterized groove library.** Shipped as
      `skills/jazz-composer/lib/grooves.wilios` (`swing_ride`, `kit_swing`,
      `kit_bossa`, `kit_shuffle`, `walk_bar`, `two_feel`, `bossa_bass2`) +
      `skills/jazz-composer/lib/comp.wilios` (comping cells).
- [x] **P2 / S — Expand `lib/lib.wilios` presets.** Added `upright` (rounder
      fingered acoustic bass), `ride` (sustained cymbal wash, distinct from
      `hihat_c`), `brushes` (soft snare swish), and `comp_piano` (sustaining
      comp voice, unlike the percussive `epiano`) — 14 presets total (`trumpet` added later). Synced
      across `lib/lib.wilios`, `wilios_core::stdlib::PRESETS`, and
      `doc/stdlib.md` (the `stdlib_doc_consistency` / `stdlib_examples` tests
      enforce it). Also fixed `transpose` to keep flat spellings for flat
      inputs (`transpose(Eb4, 0)` → `Eb4`, not `D#4`) and to error rather than
      silently clamp below C0.

## 6. MCP & tooling

- [ ] **P1 / M — MCP `analyze` tool:** given source, return bar-by-bar harmonic analysis
      plus warnings (bass note outside the chord, melody note clashing with a b9, range /
      register issues, parallel fifths/octaves).
- [ ] **P2 / S — MCP `list_presets` / `preview_preset`** (short audio of a patch) to
      complement the existing `describe_symbol`.
- [ ] **P2 / M — MCP `skeleton` tool:** given a chord progression + style, emit a starting
      `.wilios` scaffold (tracks, tempo, preset choices, form).
- [ ] **P2 / S — `scales` MCP: emit wilios pitch tokens** (`<C4> <Eb4> ...`) directly,
      not note-name strings that need hand-conversion.

## 7. Skills / docs

- [x] **P1 / S — A `wilios` authoring skill.** Shipped as `skills/jazz-composer/`:
      `SKILL.md` + `references/{workflow,harmony,voicings,rhythm-and-feel,styles,wilios-gotchas}.md`,
      with `examples/blues_f.wilios` (repo root) as its annotated teaching file;
      `make install-skills` symlinks it into `.claude/skills/`. Covers the syntax reference, every
      composer-facing gotcha (call-site `func` scoping, pitch grammar
      `LETTER[#|b]OCTAVE`, `tempo`/`volume`/`pan`
      literal-int-only, restate `tempo`/`swing`/`time_signature` per track), a
      validated starter template, and an idiom cookbook.

  > **Two deltas from the original idea.** (a) It is scoped to **jazz**, not a
  > neutral `wilios` skill — the general DSL material is embedded but jazz-framed;
  > extract a standalone `wilios` skill only if a non-jazz authoring need shows up.
  > (b) The loop shipped as `validate → dump → render`, not `validate → render →
  > analyze` — the MCP `analyze` (§6) and MCP `render` (§1) tools don't exist yet;
  > fold them into `SKILL.md`'s "feedback loop" section when they land.
- [x] **P2 / S — Style packs** (bebop / bossa / blues / modal): characteristic
      progressions, rhythms, preset choices, tempo ranges — as importable snippets +
      skill notes. Skill-notes half: `skills/jazz-composer/references/styles.md`
      (eight styles). Importable-snippet half: `skills/jazz-composer/lib/{bebop,
      bossa,blues,modal}.wilios` — each carries the style's settings in a header
      block, re-imports the idiom files, and adds signature helpers; each has a
      worked demo `examples/<style>_from_lib.wilios` wired into `make smoke` / CI.
      `styles.md` and `SKILL.md` point at them; `references/idiom-library.md` is
      the index.
- [x] **P2 / S — Idiom cookbook resource:** ii–V licks, walking-bass formulas, comping
      rhythms, drum patterns by style, as importable `.wilios` files. Shipped as
      `skills/jazz-composer/lib/{licks,comp,grooves}.wilios` (the prose snippets
      in `references/{rhythm-and-feel,harmony,voicings}.md` stay as the
      explanation). Enabled by the §2 func-composition fix; a resolver
      false-positive on func-body-local `let`s was fixed alongside
      (`resolve::scope`).

---

## Quick wins (do first)

1. ~~`wilios render` to WAV + `wilios dump` (§1)~~ — **done**; unblocked the rest.
2. ~~Allow `func` → `func` calls (§2)~~ — **done**; removed the most awkward language limitation.
3. `lib/theory.wilios` (§5) — pure library, ships without engine changes.
4. ~~A `wilios` authoring skill (§7)~~ — **done** as `skills/jazz-composer/`.

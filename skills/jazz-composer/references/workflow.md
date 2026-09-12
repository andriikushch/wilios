# Composition workflow

The full method behind the short spine in `SKILL.md`. Work the steps in order;
skip lightly for a short phrase, do all of them for a full tune. Do the
self-review (step 10) silently before presenting anything.

## Input contract

Extract whatever the request gives; infer sensible defaults for the rest and
state them in one line. Ask a question only when a missing field would
materially change the piece.

```
title        style         era          mood
tempo_bpm    meter         key          tonal_center
duration     form          ensemble     reference
complexity   harmonic_language   melodic_language   rhythmic_language
constraints  deliverables
```

## Step 1 — Musical brief

Summarize the intended identity in 1–3 sentences. Example: "Medium-up post-bop
quartet, nocturnal, lyrical head that gains rhythmic density toward the bridge,
no textbook ii–V cadence." Do not over-explain.

## Step 2 — Form

Pick a form that fits the idea and **write the bar counts explicitly**:

```
12-bar blues · 16-bar blues · 32-bar AABA · ABAC · rhythm changes ·
modal cycle · vamp + head · odd-meter repeated form · through-composed
```

```
A   8 bars
A'  8 bars
B   8 bars   (bridge)
A'' 8 bars
tag 2 bars
```

In wilios the form is literal: one phrase `func` per section, called in order
inside the track (see `examples/bebop_trio.wilios` — `a7() end_turn() a7()
end_bridge() bridge() a7() end_final()`).

## Step 3 — Harmonic architecture

Decide tonal center(s), harmonic rhythm, primary cadences, points of departure
and arrival, dominant activity, modal areas, chromatic regions. Do **not** add
substitutions just to thicken the chart. Detail and idioms:
[`harmony.md`](harmony.md).

## Step 4 — Motif

Create one or two small identifiable cells. Specify interval content, rhythmic
cell, contour, characteristic accent. Then develop it — change rhythm, interval,
register, contour, harmonization, density, or displace it. Prefer development
over a string of unrelated ideas.

## Step 5 — Melody

Build phrases that interact with the harmony: chord tones on structurally strong
beats, tensions and chromatic approach tones as language (not decoration),
passing tones, rests, sequence, repetition with variation. Not every note a
chord tone; not every note a tension. Give phrases clear lengths and direction.

## Step 6 — Rhythm / feel

Define the rhythmic identity: subdivision, syncopation, anticipation, backbeat,
metric displacement, space. In odd meters state the grouping (`7/8 = 2+2+3`).
Keep the rhythmic motif recognizable while you develop it. wilios specifics
(swing, tuplets, `time_signature`): [`rhythm-and-feel.md`](rhythm-and-feel.md).

## Step 7 — Bass

Define the bass **role**, not just roots: walking, pedal, ostinato, counterline,
anticipatory, sparse/modal, contrapuntal. Walking-bass construction is in
[`rhythm-and-feel.md`](rhythm-and-feel.md).

## Step 8 — Arrangement

Assign roles per track. A quartet head might be: lead → melody, comp → harmonic
commentary, bass → groove/counterline, drums → time + interaction. Do not make
every instrument play continuously. Ensemble defaults per style:
[`styles.md`](styles.md).

## Step 9 — Improvisational architecture

If the piece has solo space, design it: number of choruses, density progression,
motif reuse, harmonic landmarks, interaction points, a climax, a release. In
wilios a "solo" is either written out or a bounded `loop` over comp/bass/drums
phrases with the lead track left thin or silent.

## Step 10 — Self-review (silent, before output)

- Does the melody have a recognizable identity?
- Does the harmony support the melody at every strong beat?
- Enough repetition **and** enough contrast?
- Is the harmonic complexity justified by the idea?
- Does the form create direction? Do the bar counts add up?
- Are chord symbols (in the comments) consistent, not randomly respelled?
- Are the ranges plausible for the presets used?
- Does the ending make musical sense?
- **wilios:** does every track restate `tempo`/`swing`/`time_signature`? Are
  helper/phrase `func`s defined at **global** scope (so call-site resolution
  finds them, and no track-local `let` shadows one)?

If something fails, revise it before presenting. If the request is deliberately
unconventional, preserve that rather than "correcting" it.

## Step 11 — Render it

Write the `.wilios` file per the output contract in `SKILL.md`, then run the
core loop: `dump --format roll`, read it, revise, re-`dump`. Optionally `render`
a WAV to hear it, and `midi` a Standard MIDI File if the user wants a DAW file.

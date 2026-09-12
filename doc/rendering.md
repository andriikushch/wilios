# Offline renderers and endless input

Everything that turns a `.wilios` source into output *without* opening an audio
device goes through one shared, device-independent path in the `wilios-render`
crate (`pipeline` → `voices` → `render`, plus `analysis` and `image`). Six
consumers sit on top of it:

| Consumer | What it produces |
| --- | --- |
| `wilios dump FILE` | per-track note-event timeline (JSON / text / ASCII piano roll) |
| `wilios render FILE -o OUT.wav` | a 16-bit stereo WAV |
| `wilios midi FILE -o OUT.mid` | a Standard MIDI File |
| `wilios smoke FILE` | headless CI check: schedules cleanly, tracks end together |
| `dump_events` MCP tool | the same timeline as `wilios dump` |
| `render` MCP tool | WAV + waveform PNG + spectrogram PNG + scalar analysis |

A `.wilios` piece can run forever — `loop (true) { … }`, or a note shorter than
its attack that never releases. Every renderer above is therefore bounded. How
it *reports* hitting that bound is the one place the CLI and the MCP tools
deliberately differ.

## The two guards

1. **A composition-time bound.** How many milliseconds of the piece's own
   timeline to schedule before stopping.
   - CLI: `--duration <seconds>`. With no `--duration`, a 600 s safety cap
     applies and *not finishing within it is an error* (see below).
   - MCP: `max_ms` (default 60000), always clamped to `[1000, 600000]`.
2. **The interpreter step cap.** `Interpreter::schedule_until` walks each track
   statement by statement and breaks out after `MAX_STEPS = 1_000_000` steps per
   track per call. This guarantees the scheduler returns even for a loop that
   never advances musical time (`loop (true) { i = i + 1 }`), where the
   composition-time bound alone would never be reached.

The `render` MCP tool adds a third, outer backstop: a **30 s wall-clock timeout**
(`dump_events` uses 5 s). It catches the pathological silent loop, whose
per-call step cap makes each buffer slow without ever producing audio or
finishing.

## Two shapes of "endless"

- **Note-emitting loop** — `loop (true) { <C4> 1/4 }`. Musical time advances, so
  the composition-time bound (guard 1) stops it. Output is real, just truncated.
- **Silent / non-advancing loop** — `loop (true) { i = i + 1 }`. Musical time is
  frozen; guard 2 returns each call with no new events, and on the MCP `render`
  tool the wall-clock timeout is ultimately what ends it.

## Per-renderer contract

| Renderer | Bound | No bound given + piece doesn't finish | Bound given + piece doesn't finish |
| --- | --- | --- | --- |
| `wilios dump` | `--duration`, else 600 s cap | **hard error**: "piece did not finish … Re-run with --duration" | dumps the window, `finished: false` |
| `wilios render` | `--duration`, else 600 s cap | **hard error**, no file written | renders exactly `--duration`, WAV written |
| `wilios midi` | `--duration`, else 600 s cap | **hard error**, no file written | exports the window |
| `wilios smoke` | `--duration`, else 600 s cap | **hard error** | schedules the window; the equal-end-time check is skipped |
| `dump_events` (MCP) | `max_ms` + 5 s wall clock | `finished: false`, events truncated — **successful call** | n/a (always bounded) |
| `render` (MCP) | `max_ms` + 30 s wall clock | note-emitting: `finished: false`, WAV/PNG/analysis truncated — **successful call**. silent: 30 s timeout → **tool error** | n/a (always bounded) |

## Render progress

`wilios render` draws a one-line progress bar on **stderr** while it synthesises,
redrawn in place (`\r`, no newline) at ~10 fps. With `--duration` it shows a true
percentage and `elapsed / total` seconds; without one, only the elapsed audio
seconds (there is no known total — see the 600 s cap above). It is shown only
when stderr is a terminal, so piped or redirected output keeps just the final
`Rendered …` line; `--no-progress` turns it off explicitly. The bar lives in
`wilios-cli` (`progress::RenderBar`); `wilios-render` only exposes a
`render_with_progress` / `render_to_samples_with_progress` callback and never
writes to a terminal itself, so the `render` MCP tool is unaffected.

## Why the MCP tools don't error

An interactive user who sees "Re-run with --duration" just does that. An agent
calling a tool has no such prompt, and a hard error throws away the partial
render it could have learned from. So the MCP tools always carry a bound and
hand back `finished: false` with truncated output, letting the caller decide
whether the truncation matters. `isError` on those tools stays reserved for
things the caller got wrong: bad arguments, an unreadable or sandbox-escaping
path, source that doesn't compile, or the wall-clock timeout.

# wilios-mcp

MCP server for the [wilios](../../README.md) music DSL. Exposes wilios documentation and examples as resources, plus stdlib lookup tools, so AI clients (Claude Code, Claude Desktop, etc.) can read and write valid `.wilios` files without inventing stdlib functions that don't exist.

## Resources

| URI | Contents |
|-----|----------|
| `wilios://docs/language-reference` | Complete language reference (Markdown) |
| `wilios://docs/grammar` | Formal EBNF grammar |
| `wilios://lib/presets` | FM preset library (`lib/lib.wilios`) |
| `wilios://examples/full-piece` | Multi-track composition example |
| `wilios://examples/swing` | Swing/feel example |

All resources are embedded at compile time, so the binary works from any working directory and has no runtime file dependencies.

## Tools

| Tool | Arguments | Description |
|------|-----------|--------------|
| `describe_symbol` | `name: string` | Look up a wilios stdlib symbol (one of the 4 built-in functions or 14 FM presets) by exact name. Returns its signature (if a function), description, and a minimal runnable example. Unknown names return an error result with near-match suggestions. |
| `search_stdlib` | `query: string` | Case-insensitive substring search over stdlib symbol names and descriptions. Returns a (possibly empty) list of matches. |
| `validate` | `source: string` \| `path: string` (exactly one), plus optional `follow_imports`, `suggestions`, `lints`, `max_diagnostics`, `excerpt` | Statically checks a wilios source — lexes, parses, resolves every identifier, and runs a handful of decidable semantic checks/lints — without rendering or executing it. Returns structured diagnostics: a stable code, a span (line/column/byte offset), an optional source excerpt with a caret, and up to 3 ranked "did you mean" suggestions for unknown identifiers. Invalid source is a *successful* call with `ok: false` in the result; `isError` is reserved for tool-level failure (bad arguments, an unreadable file, a path escaping the sandbox, or a timeout). |
| `dump_events` | `source: string` \| `path: string` (exactly one), plus optional `max_ms` and `format` (`"json"` default, or `"roll"`) | Runs the source through the interpreter and returns its per-track note-event timeline: onset (`at_ms` and exact `at_beats`), pitch spelling, MIDI note number and frequency, duration, velocity, pan, waveform, ADSR, and full FM operator config. `format: "roll"` returns a compact ASCII piano roll as text instead of the JSON timeline. This is one of two tools that execute the interpreter; it opens no audio device and is bounded by `max_ms` of composition time (default 60000, clamped to [1000, 600000]) and a 5 s wall-clock timeout. A piece that does not finish in that budget comes back with `finished: false` and its events truncated at the bound — a *successful* call, not an error; `isError` is reserved for tool-level failure (bad arguments, an unreadable/sandbox-escaping path, a timeout, or source that does not compile — run `validate` for diagnostics in that case). |
| `render` | `source: string` \| `path: string` (exactly one), plus optional `max_ms`, `sample_rate`, `want` (subset of `["audio","waveform","spectrogram","analysis"]`), `inline` | Synthesises the piece offline (no audio device) and returns what it *sounds* like: the WAV, a waveform PNG, a log-frequency spectrogram PNG, and a compact `analysis` block (peak/RMS dBFS, clipped-sample count, limiter ratio, leading/trailing silence, per-track note count and pitch range). Audio and images come back as `wilios://render/<id>.{wav,png}` **resource links** — read them with `resources/read` — unless `inline: true` and the payload is under 256 KiB, in which case they are base64 `audio`/`image` blocks. `used_rng: true` flags a piece whose `rand(...)` calls make renders non-reproducible (there is no seeded-RNG mode yet). Bounded by `max_ms` (default 60000, clamped [1000, 600000]) and a 30 s wall clock. See [Endless input](#endless-input) for the non-termination contract. |

`describe_symbol`/`search_stdlib` are backed by a machine-readable symbol table in `wilios-core` (`wilios_core::interpreter::BUILTINS`, `wilios_core::stdlib::PRESETS`) — the 4 builtins' table entries double as their actual runtime registration, so they can't drift from what the interpreter really supports. Every symbol's example is verified to actually run, and names/signatures/categories are checked against `doc/stdlib.md` for consistency, both under `cargo test --workspace` (see `crates/wilios-core/tests/stdlib_examples.rs` and `stdlib_doc_consistency.rs`). `validate`'s analysis pass (`wilios_core::resolve`) reuses that same table for its "did you mean" suggestions and builtin-arity checks, and reuses the interpreter's own import-sandboxing rules (`wilios_core::parser::parser::resolve_import_path`) for the `path` argument and for following `import` statements. `dump_events` reuses that same `resolve_import_path` sandboxing, and its event → JSON mapping is `wilios_core::dump` (behind the crate's `serde` feature), driven by `Interpreter::schedule_to_end` — the same code path `wilios dump` uses, so the two can't drift. `render` reuses the same sandboxing and drives the device-independent render path in `wilios-render` (`render_to_samples` + `analysis` + `image`), which `wilios render`/`wilios play` also consume; CI (`no-cpal-in-core`) enforces that neither `wilios-render` nor `wilios-mcp` links `cpal`.

## Endless input

`render` and `dump_events` both always carry a bound and, on a non-terminating piece, return a *successful* result with `finished: false` and output truncated at that bound — never a tool error (unless the piece is genuinely silent and only the 5 s / 30 s wall-clock timeout stops it). This is deliberate: an agent has no interactive prompt to re-run with `--duration`, so a truncated result it can inspect beats a refusal. The CLI renderers (`wilios dump`/`render`/`midi`/`smoke`) instead hard-error and ask for `--duration`. Full contract and rationale: [`doc/rendering.md`](../../doc/rendering.md).

## Build

```bash
# From the workspace root:
cargo build --release -p wilios-mcp

# Binary is at:
./target/release/wilios-mcp
```

## Configuring Claude Code

```bash
claude mcp add --scope project wilios -- /absolute/path/to/target/release/wilios-mcp
```

This writes to `.mcp.json` at the project root, which can be committed so everyone on the team gets the server automatically.

To register it for yourself only (not committed):

```bash
claude mcp add wilios -- /absolute/path/to/target/release/wilios-mcp
```

Verify the server is connected:

```bash
claude mcp list
```

## Configuring Claude Desktop

Add the following to `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "wilios": {
      "command": "/absolute/path/to/target/release/wilios-mcp",
      "args": []
    }
  }
}
```

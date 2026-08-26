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
| `describe_symbol` | `name: string` | Look up a wilios stdlib symbol (one of the 4 built-in functions or 9 FM presets) by exact name. Returns its signature (if a function), description, and a minimal runnable example. Unknown names return an error result with near-match suggestions. |
| `search_stdlib` | `query: string` | Case-insensitive substring search over stdlib symbol names and descriptions. Returns a (possibly empty) list of matches. |

Both tools are backed by a machine-readable symbol table in `wilios-core` (`wilios_core::interpreter::BUILTINS`, `wilios_core::stdlib::PRESETS`) — the 4 builtins' table entries double as their actual runtime registration, so they can't drift from what the interpreter really supports. Every symbol's example is verified to actually run, and names/signatures/categories are checked against `doc/stdlib.md` for consistency, both under `cargo test --workspace` (see `crates/wilios-core/tests/stdlib_examples.rs` and `stdlib_doc_consistency.rs`).

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

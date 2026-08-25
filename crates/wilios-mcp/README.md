# wilios-mcp

MCP server for the [wilios](../../README.md) music DSL. Exposes wilios documentation and examples as resources so AI clients (Claude Code, Claude Desktop, etc.) can read and write valid `.wilios` files.

## Resources

| URI | Contents |
|-----|----------|
| `wilios://docs/language-reference` | Complete language reference (Markdown) |
| `wilios://docs/grammar` | Formal EBNF grammar |
| `wilios://lib/presets` | FM preset library (`lib/lib.wilios`) |
| `wilios://examples/full-piece` | Multi-track composition example |
| `wilios://examples/swing` | Swing/feel example |

All resources are embedded at compile time, so the binary works from any working directory and has no runtime file dependencies.

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

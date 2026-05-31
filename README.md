# so-context

> The context management layer for AI agents. One tool, zero config, every token optimization technique working together.

---

## What it is

AI coding agents burn tokens on things that don't need to be in context: full file contents read repeatedly, raw shell output, bloated MCP tool descriptions, unindexed codebases scanned line by line.

Five separate techniques exist to fix this. They all work. None of them talk to each other.

**so-context is the unified layer.** Install once, every technique applies automatically.

---

## Current capabilities

| Technique | Status |
|---|---|
| Code graph indexing | ✅ |
| Incremental sync + multi-project watch | ✅ |
| Multi-agent auto-discovery | ✅ |
| Repeat-read deduplication | Planned |
| Shell output compression | Planned |
| Large-output FTS offloading | Planned |
| MCP tool description shrinking | Planned |
| MCP stdio transparent proxy | Planned |

---

## Usage

### Daemon (MCP server)

```sh
cargo run -- daemon
```

Add to your agent's MCP config:

```json
{
  "mcpServers": {
    "so-context": {
      "command": "so-context",
      "args": ["daemon"]
    }
  }
}
```

The daemon auto-discovers your project on first tool call — no path configuration needed.

### One-shot index

```sh
cargo run -- index [path]
```

### Foreground watch (single project)

```sh
cargo run -- watch [path]
```

### Shell compression

```sh
cargo run -- shell -- git diff
cargo run -- shell --full -- git diff
```

Supported commands are tracked in [docs/shell-support.md](/Users/jiatwork/Works/so-context/docs/shell-support.md).

---

## MCP tools

| Tool | Description |
|---|---|
| `so_read` | Read a file. `mode`: `full` (default), `outline`, `graph` (triggers index) |
| `so_search` | FTS search over the indexed code graph |
| `so_status` | List all auto-discovered projects and their sync state |

---

## Development

```sh
cargo build
cargo check
cargo run -- daemon
```

---

## License

MIT

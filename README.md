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
| Shell output compression | ✅ |
| Large-output FTS offloading | Planned |
| MCP tool description shrinking | Planned |
| MCP stdio transparent proxy | Planned |

---

## Installation

```sh
# Primary install path
curl -fsSL https://raw.githubusercontent.com/vestin-io/so-context/main/scripts/install.sh | sh
```

The install script downloads a prebuilt GitHub release binary for:
- macOS Apple Silicon (`aarch64-apple-darwin`)
- macOS Intel (`x86_64-apple-darwin`)
- Linux x86_64 (`x86_64-unknown-linux-gnu`)

If no matching release asset exists yet, it falls back to building from source with `cargo`.

Developer fallback options:

```sh
# Homebrew stable formula (requires repository access; builds from tagged source)
brew install ./Formula/so-context.rb

# Homebrew development build from main
brew install --HEAD ./Formula/so-context.rb

# Cargo (builds from source)
cargo install --git https://github.com/vestin-io/so-context so-context
```

After install:

```sh
so-context setup
```

`setup` installs the MCP server, registers agent hooks, and adds global agent instructions that prefer `so_shell` for short, one-shot shell commands.
Short native shell calls are blocked and should be retried through the MCP `so_shell` tool.

---

## Usage

### Daemon (MCP server)

```sh
so-context daemon
```

Add to your agent's MCP config:

```json
{
  "mcpServers": {
    "so-context": {
      "command": "so-context",
      "args": ["mcp"]
    }
  }
}
```

The daemon auto-discovers your project on first tool call — no path configuration needed.

### One-shot index

```sh
so-context index [path]
```

### Foreground watch (single project)

```sh
so-context watch [path]
```

## MCP tools

| Tool | Description |
|---|---|
| `so_read` | Read a file. `mode`: `full` (default) or `outline` |
| `so_search` | FTS search over the indexed code graph |
| `so_references` | Find callers, callees, imports, or all references for a named symbol |
| `so_shell` | Run a local shell command and return compressed output |
| `so_status` | List all auto-discovered projects and their sync state |

---

## Development

```sh
cargo build
cargo check
so-context daemon
```

## Maintainer release flow

1. Bump `Cargo.toml` version.
2. Commit the release changes.
3. Tag the release with a `v` prefix, for example `v0.1.1`.
4. Push the branch and tag:

```sh
git push origin main
git push origin v0.1.1
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds and uploads:
- `so-context-x86_64-unknown-linux-gnu.tar.gz`
- `so-context-x86_64-apple-darwin.tar.gz`
- `so-context-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS.txt`

You can also run the same workflow manually from GitHub Actions with `workflow_dispatch`:
- optional `version` input, for example `0.1.1`
- optional `draft` toggle

If the manual dispatch `version` is empty, the workflow falls back to the version in `Cargo.toml`.

---

## License

MIT

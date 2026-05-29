# so-context Engineering Guide

This file defines repository-level engineering rules for humans and coding agents.

## 0. Scope

- This `AGENTS.md` applies to the whole repository unless a deeper `AGENTS.md` overrides it.
- Product scope, priorities, and execution order should come from the active issue tracker or explicit user instructions.

## 1. Purpose

- Build `so-context` as a local-first Rust runtime for AI-agent context management.
- Keep the current implementation centered on:
  - MCP stdio server
  - project auto-discovery
  - SQLite-backed code graph indexing and search
  - read/search/status tools exposed to agent hosts
- Keep the longer-term product direction compatible with:
  - MCP proxy / interception
  - shell output compression
  - large-output offloading and retrieval
  - file/session cache layers
- Prefer boring, reversible design decisions over speculative framework work.

## 2. Hard Rules

- Hand-written non-`json` and non-`md` files MUST stay under **500 lines**.
- If a source file approaches **400 lines**, split it before adding more behavior.
- Generated files, fixtures, and schema artifacts are exceptions only when their origin is obvious from the path or filename.
- Prefer not to introduce new crates or binaries unless the current single-binary layout is clearly insufficient.
- Prefer not to mix CLI dispatch, daemon lifecycle, MCP protocol handling, graph storage, and read-mode logic in one module.
- Do not add network- or model-dependent logic to indexing, graph search, or read paths without explicit user direction.
- Avoid hiding side effects behind vague convenience helpers.

## 3. Repository Structure

- `src/main.rs`
  - Clap-based CLI entrypoint only.
  - Owns command parsing and top-level dispatch.
- `src/daemon/`
  - Long-lived daemon services and project watch management.
  - Owns auto-discovery and watcher lifecycle.
- `src/mcp/`
  - MCP server protocol surface only.
  - Owns tool routing, handshake hooks, and stdio transport integration.
- `src/mcp/tools/`
  - Individual MCP tool handlers and schemas.
- `src/core/read.rs`
  - File read modes and file-read shaping logic.
- `src/core/graph/`
  - Code graph storage, indexing, sync, watch helpers, symbol extraction, and search.
- Future seams that are allowed when the product grows:
  - `src/proxy/` for MCP proxy / interception logic
  - `src/cache/` for file/session/output cache layers
  - `src/compress/` for deterministic shell or tool-output compression

## 4. File and Module Organization

- Prefer one module per responsibility.
- Prefer `foo.rs` over `foo/mod.rs` for new modules unless a directory already owns clear submodules.
- Keep nesting shallow.
- Public module boundaries should be obvious from `main.rs` or the parent `mod.rs`.
- Avoid placeholder names such as `misc`, `common`, `manager`, `helper`, or `util` unless the file truly owns that concept.
- If a directory contains only one file and adds no real abstraction, collapse it.

## 5. Rust Style

- Follow `rustfmt` output. Do not hand-format around it.
- Use `snake_case` for value-level items and `UpperCamelCase` for type-level items.
- Getter methods should follow Rust conventions:
  - `path()`, not `get_path()`
  - `first_mut()`, not `get_first_mut()`
- Conversion methods should follow standard Rust meaning:
  - `as_*` for cheap borrowed views
  - `to_*` for potentially expensive conversion
  - `into_*` for ownership-consuming conversion
  - `into_inner()` for wrappers exposing the wrapped value

## 6. API Design Rules

- Favor small, explicit types over overloaded helper functions.
- Use the type system to encode meaning; avoid boolean argument soup.
- Return structs instead of tuples when fields have domain meaning.
- Constructors should be inherent methods such as `new`, `open`, or `from_path`.
- Keep APIs private by default and expose only what another module actually needs.
- Do not pass parameters through multiple layers just to forward them. If an intermediate layer does not use a value, restructure the wiring.
- Return concrete domain objects instead of loose maps or ad hoc stringly typed metadata.

## 7. Error Handling

- Use `Result` for expected runtime failures.
- Use `panic!` only for programmer bugs or impossible states.
- Prefer explicit error handling in non-test code, but `unwrap()` / `expect(...)` are acceptable for bootstrap invariants, poison-path handling, or cases where crashing is the clearest failure mode.
- Error messages must explain the failed operation and the missing assumption.
- At module boundaries, return actionable errors with context.
- Avoid discarding errors silently when there is a reasonable recovery or reporting path.

## 8. Async and Runtime Rules

- `Tokio` is the async runtime for this repository.
- `src/daemon/` owns daemon lifecycle and long-lived orchestration.
- `src/mcp/` may be async, but it must not own graph storage rules or watcher policy.
- Blocking work must stay isolated:
  - watcher threads may remain dedicated OS threads when required by `rusqlite` or file watching constraints
  - async request handlers should avoid hidden blocking work
- Prefer to make ownership, shutdown, and failure behavior explicit for background tasks.
- Prefer explicit request/response flow over hidden global behavior when practical.

## 9. Graph and Storage Rules

- SQLite is the default local persistence layer.
- Graph data should live under the repository-local `.so-context/` directory.
- Schema initialization must happen automatically.
- Prefer to keep a clean boundary between:
  - DB/schema/path helpers
  - indexing/sync logic
  - symbol extraction
  - search/result formatting
- Changes to SQLite schema, graph layout, or persisted daemon state should account for upgrade paths from older local state when possible.
- Secrets must not be written into SQLite, logs, fixtures, or docs.

## 10. MCP and Tooling Rules

- MCP tool registration belongs in `src/mcp/tools/`.
- Each tool should own:
  - schema
  - argument extraction
  - domain call
  - response shaping
- Prefer not to bury reusable core logic in tool handlers when it clearly belongs in `src/core/` or `src/daemon/`.
- Tool descriptions should stay concise and operational.
- Prefer to keep auto-discovery behavior explicit in the MCP handshake and tool-call hooks instead of scattering root-registration side effects through unrelated code.
- Prefer to keep `src/mcp/mod.rs` as a server surface rather than a dumping ground for future proxy, cache, or compression logic.

## 11. Proxy and Compression Rules

- Prefer to keep proxy/interception logic separate from graph storage and read-mode logic.
- Proxy layers may inspect and reshape protocol traffic, but should avoid quietly absorbing core domain behavior that belongs in `src/core/`, `src/daemon/`, or dedicated capability modules.
- Compression should be deterministic by default.
- Compression should not require network access or model calls unless the user explicitly asks for that direction.
- Shell or tool-output compression should preserve raw meaning unless a capability is explicitly designed as lossy and documented as such.
- Large-output offloading, cache references, and summary responses should keep a recoverable path back to the underlying content.
- Planned capabilities such as proxying, compression, caching, and graph indexing should compose through clear module boundaries instead of being merged into one catch-all runtime file.

## 12. Read and Graph Rules

- Read modes should degrade safely when graph data is missing or stale.
- Prefer not to let file-read logic quietly become a second indexing pipeline.
- Graph search and context shaping should prefer deterministic output over clever heuristics.
- New graph features should usually land in this order:
  - stable storage or symbol model
  - indexing/sync correctness
  - query surface
  - richer ranking or formatting

## 13. Testing and Verification

- Prefer focused unit tests for substantive modules.
- Runtime changes should include an end-to-end path test where practical.
- Graph changes should add tests for indexing/search behavior when feasible.
- Before considering work done, try to run:
  - `cargo fmt --all --check`
  - `cargo check`
  - `cargo build`
  - `cargo test`
- Run `cargo clippy` when changing shared runtime, MCP, or graph code if practical.
- MCP changes should add or update tests for handshake behavior, tool routing, or auto-discovery when practical.
- If any verification step is skipped, record the reason in the handoff, commit message, or PR description.

## 14. Dependency Rules

- Add dependencies only where they have a direct, clear use.
- Avoid large frameworks when a focused crate is enough.
- New dependencies must have a clear purpose tied to one of:
  - CLI
  - daemon/runtime
  - MCP protocol handling
  - proxy/compression/cache
  - graph/indexing
  - storage

## 15. Documentation Rules

- Keep `README.md` high-level and operational.
- Keep repo-tracked documentation limited to durable engineering guidance and operational docs.
- Public-facing or shared types should have concise rustdoc comments when their meaning is not obvious.
- If a function can fail in non-obvious ways, document the failure mode.
- Do not add repo-tracked planning artifacts unless the user explicitly asks for them.
- When handing work off, record key findings, deliberate omissions, known gaps, and follow-up risks in the final update or PR notes.

## 16. Change Management

- Small vertical slices beat wide placeholder scaffolding.
- Prefer not to implement speculative abstractions before the next real caller exists.
- If a file crosses the size rule, split by responsibility before continuing.
- If a module starts coordinating multiple unrelated concerns, prefer introducing a clearer module boundary.
- Prefer removing dead code instead of keeping unused fields, types, functions, or modules as placeholders.
- If a rule here would force a worse local design, choose the simpler design and record the exception in the handoff.

## 17. References

These rules intentionally align with the primary Rust guidance below:

- Rust API Guidelines checklist:
  - https://rust-lang.github.io/api-guidelines/checklist.html
- Rust API naming guidance:
  - https://rust-lang.github.io/api-guidelines/naming.html
- Rust style guide / rustfmt style editions:
  - https://doc.rust-lang.org/stable/style-guide/editions.html
- Cargo reference:
  - https://doc.rust-lang.org/cargo/reference/
- Rust error handling (`std::error`):
  - https://doc.rust-lang.org/std/error/
- Clippy documentation:
  - https://doc.rust-lang.org/clippy/
- Rust module/file organization:
  - https://doc.rust-lang.org/reference/items/modules.html
  - https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html

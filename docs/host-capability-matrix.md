# Host Capability Matrix

This document captures the current host-specific integration facts that matter
for the `so-context` runtime refactor.

It is intentionally practical:

- what the host calls tools
- which lifecycle surfaces are currently wired
- where session-like identity comes from
- which config surfaces setup must touch

This is a reference document for implementing `AgentAdapter` and
`SetupRenderer`.

## Scope

Current supported hosts:

- Codex
- Claude
- OpenCode

## Matrix

| Capability | Codex | Claude | OpenCode |
|---|---|---|---|
| Current integration style | MCP + hooks + `AGENTS.md` include | MCP + hooks + `CLAUDE.md` rules include | MCP + generated plugin |
| Current setup file(s) | `~/.codex/config.toml`, `~/.codex/AGENTS.md`, `~/.codex/SO-CONTEXT.md` | `~/.claude/settings.json`, `~/.claude/CLAUDE.md`, `~/.claude/rules/so-context.md` | `~/.config/opencode/opencode.json`, `~/.config/opencode/plugins/so-context.ts` |
| Native pre-tool interception surface | `PreToolUse` hook | `PreToolUse` hook | `tool.execute.before` plugin event |
| Current compact/reset surface | `PostCompact` hook | `PostCompact` hook | current repo plugin uses `session.compacted`; official docs clearly document `experimental.session.compacting` for compaction customization |
| Current instruction surface | `AGENTS.md` include | `CLAUDE.md` + `@rules/...` | plugin-generated runtime messages only; no separate markdown instruction file today |
| Current MCP tool naming shape | `mcp__so-context__<tool>` | `mcp__so-context__<tool>` | `so-context_<tool>` |
| Current native read aliases | `Read`, `read`, `View`, `view`, `read_file` | `Read`, `read`, `View`, `view`, `read_file` | `Read`, `read`, `View`, `view`, `read_file` |
| Current native search aliases | `Grep`, `grep`, `rg`, `ripgrep`, `SearchFiles`, `search_files` | `Grep`, `grep`, `rg`, `ripgrep`, `SearchFiles`, `search_files` | `Grep`, `grep`, `rg`, `ripgrep`, `SearchFiles`, `search_files` |
| Current native shell aliases | `Bash`, `bash`, `Shell`, `shell`, `runTerminalCommand`, `runInTerminal`, `run_in_terminal`, `terminal`, `shell_command`, `exec_command`, `local_shell`, `run_shell_command` | same as Codex | shared shell alias list rendered into the generated plugin |
| Current self-tool session injection | Rust hook returns `updatedInput._so_session_id` | Rust hook returns `updatedInput._so_session_id` | plugin applies shared hook `updatedInput`; self-tool fallback injects `_so_session_id` only if hook invocation fails |
| Current session-like identity source | `session_id`, fallback `agent_id` in hook payload; MCP connection separately tracks `client_info` and generated `connection_id` | `session_id`, fallback `agent_id` in hook payload; `PostCompact` also carries `connection_id` | `input.sessionID` in plugin event; compact event reads `input.session.id` or `input.sessionID` |
| Current block/deny mechanism | hook returns `permissionDecision: \"deny\"` with reason | hook returns `permissionDecision: \"deny\"` with reason | plugin throws an error with retry guidance |
| Current rewrite/enrich mechanism | hook returns `permissionDecision: \"allow\"` with `updatedInput` | same as Codex | plugin mutates `output.args` directly |
| Current watch registration path | daemon auto-registers project roots from MCP initialize / roots | same shared daemon path | same shared daemon path |
| Current optional host surfaces not yet used by so-context | `SessionStart`, `PermissionRequest`, `PostToolUse`, `UserPromptSubmit`, `SubagentStart`, `SubagentStop` | `SessionStart`, `PostToolUse`, `InstructionsLoaded`, `PreCompact`, `SessionEnd` | `tool.execute.after`, `permission.*`, `message.*`, `experimental.session.compacting` |

## Real Integration Notes

### Codex

- Hook payloads are stdin JSON and responses are stdout JSON.
- `PreToolUse` currently supports deny or rewrite for supported tool calls.
- MCP tool names arrive in `mcp__server__tool` format.
- Hook matchers are configured in `config.toml`.
- Trust/review behavior exists for hooks in Codex and should be treated as a
  real setup concern.

Primary references:

- [src/setup/codex.rs](/Users/jiatwork/Works/context/so-context/src/setup/codex.rs:1)
- [src/hook/pre_tool.rs](/Users/jiatwork/Works/context/so-context/src/hook/pre_tool.rs:32)
- [Codex Hooks](https://developers.openai.com/codex/hooks)

### Claude

- Hook payload shape is close enough to Codex that the current Rust pre-tool
  handler is shared.
- `PostCompact` currently matters more for Claude because the compact-reset hook
  explicitly carries both `session_id` and `connection_id`.
- Rules are injected through `CLAUDE.md` plus `@rules/so-context.md`.

Primary references:

- [src/setup/claude.rs](/Users/jiatwork/Works/context/so-context/src/setup/claude.rs:1)
- [src/hook/post_compact.rs](/Users/jiatwork/Works/context/so-context/src/hook/post_compact.rs:1)
- [src/setup/instructions.rs](/Users/jiatwork/Works/context/so-context/src/setup/instructions.rs:70)

### OpenCode

- Interception starts inside a generated TypeScript plugin, then delegates
  pre-tool routing decisions to the shared Rust hook through the CLI bridge.
- MCP tool names are flattened to `so-context_<tool>`.
- Blocking is performed by throwing an error in `tool.execute.before`.
- Argument enrichment is performed by mutating `output.args`.
- Session identity paths and native-tool alias sets are rendered from shared
  host/runtime metadata rather than hard-coded separately inside the plugin
  template.
- The current repo plugin listens to `session.compacted` for cache reset.
- Official OpenCode docs clearly document `experimental.session.compacting` for
  compaction prompt/context customization; treat that as a separate, richer
  extension surface rather than proof that `session.compacted` is formally
  documented the same way.

Primary references:

- [src/setup/opencode.rs](/Users/jiatwork/Works/context/so-context/src/setup/opencode.rs:1)
- [src/setup/so-context.ts](/Users/jiatwork/Works/context/so-context/src/setup/so-context.ts:1)
- [OpenCode Plugins](https://opencode.ai/docs/plugins/)

## Adapter Implications

The future `AgentAdapter` layer must normalize at least these host differences:

- native hook event carrier
- tool naming strategy
- identity field extraction
- deny/rewrite mechanics
- compact/reset event source

The future `SetupRenderer` layer must normalize at least these host differences:

- config file format
- instruction injection surface
- plugin vs hook installation model
- uninstall ownership and merge behavior

## Current Gaps To Watch

- Codex and Claude share a Rust hook path today, but should still be modeled as
  separate host adapters because their long-term lifecycle surfaces differ.
- OpenCode now shares naming, identity, and routing semantics with the shared
  runtime, but still begins from a plugin entry surface rather than a native
  hook surface.
- Tool namespace mapping is host-specific and must remain explicit.
- Subagent-related surfaces exist in Codex and Claude ecosystems but are not yet
  part of phase 1 behavior.

## Fourth Host Checklist

To add a fourth host without copy-pasting policy:

1. Add a new `HostKind` and `HostCapabilityProfile`.
2. Add a concrete `AgentAdapter` that normalizes tool and compact payloads.
3. Add a concrete `SetupRenderer` only if the host needs install/uninstall
   support.
4. Reuse `RoutingService`, `SessionService`, shared routing metadata, and shared
   hook/plugin helper logic where the host surface allows it.
5. Keep host-only behavior in the adapter or renderer rather than re-encoding
   shared native-tool routing policy.

## Deferred Extension Surfaces

These host surfaces are intentionally deferred from phase 1, but the current
adapter/profile model leaves room for them:

- session start / resume
- sub-agent start / stop
- post-tool review
- permission request interception
- prompt transformation / prompt append
- richer pre-compact or compact customization hooks

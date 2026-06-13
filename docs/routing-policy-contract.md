# Routing Policy Contract

This document defines the contract for the future `RoutingService`.

It is not an implementation design for Rust or TypeScript specifically.

It defines:

- the normalized input shape
- the normalized output shape
- what gets rerouted
- what must stay native
- how policy is shared across hosts

## Goal

`RoutingService` owns one shared policy:

- detect simple native file reads
- detect simple native indexed-search candidates
- detect short non-interactive shell commands
- reroute them to `so-context` tools when doing so improves shared context

This policy should behave consistently across hosts even if the host-specific
execution path differs.

## Non-Goals

- replacing MCP tool execution
- parsing every possible shell language construct
- implementing host lifecycle hooks
- owning session continuity behavior

## Normalized Input

Every host-specific adapter should convert its native event shape into a common
request shape conceptually equivalent to:

```json
{
  "host_type": "codex|claude|opencode|future",
  "tool_namespace": "native|so-context-mcp|other-mcp",
  "tool_name": "string",
  "tool_args": {},
  "identity": {
    "session_id": "string|null",
    "agent_id": "string|null",
    "connection_id": "string|null"
  }
}
```

Notes:

- `tool_namespace` is needed because hosts expose our MCP tools under different
  names.
- `identity` is passed through for enrichment decisions but routing policy
  should not require every field to exist.

## Normalized Output

Routing returns one of these conceptual outcomes:

### 1. Allow

The host-native tool call should continue unchanged.

```json
{
  "decision": "allow"
}
```

### 2. Allow With Enrichment

The tool call should continue, but with enriched arguments.

Primary phase-1 use:

- inject `_so_session_id` for `so-context` tool calls

```json
{
  "decision": "allow",
  "updated_args": {}
}
```

### 3. Deny With Retry Guidance

The host-native tool call should be blocked and the host should be told how to
retry using a `so-context` tool.

```json
{
  "decision": "deny",
  "reason": "human-readable guidance",
  "retry_tool": "so_read|so_search|so_shell",
  "retry_args": {}
}
```

## Host Translation Rule

Hosts do not need to consume the normalized output the same way.

Examples:

- Codex / Claude hook path
  - translate deny into `permissionDecision: "deny"`
  - translate allow-with-enrichment into `updatedInput`
- OpenCode plugin path
  - translate deny into `throw Error(...)`
  - translate allow-with-enrichment into `output.args` mutation

The policy must be shared even if the host translation is not.

## Policy Classes

### A. Self-tool enrichment

When the invoked tool is a `so-context` tool:

- allow the call
- inject `_so_session_id` when a stable session-like identifier exists

Current host-specific name shapes:

- Codex / Claude: `mcp__so-context__<tool>`
- OpenCode: `so-context_<tool>`

### B. Native read reroute

Reroute when all are true:

- tool is a known native file-read alias
- a non-empty path can be extracted
- the request shape can be normalized safely

Retry target:

- `so_read`

Retry argument rules:

- preserve `path`
- preserve `mode` if present
- preserve excerpt arguments such as `start_line`, `end_line`, `line_numbers`
- default to `mode: "full"` when no excerpt-specific arguments are present

Pass through when:

- file path is missing
- host arguments do not map safely

### C. Native search reroute

Reroute when all are true:

- tool is a known native search alias
- the query is simple literal search, not regex-style
- only the allowed simple search keys are present

Retry target:

- `so_search`

Retry argument rules:

- preserve normalized `query`
- preserve `path` when present
- preserve positive `limit` when present

Pass through when:

- regex or regexp flags are present
- query includes regex-style operators
- host arguments include extra semantics we cannot preserve safely

### D. Native shell reroute

Reroute when all are true:

- tool is a known native shell alias
- command is short, one-shot, and parseable as a simple argv sequence
- policy classifies the program as safe/preferable for `so_shell`

Retry target:

- `so_shell`

Retry argument rules:

- normalize to `argv`
- preserve env-prefix commands through `env ...`

Pass through when:

- command is empty
- command uses complex shell syntax
- command is long-running, interactive, streaming, or policy-kept-native

## Known Keep-Native Categories

Current keep-native logic includes categories like:

- interactive programs
- streaming/follow modes
- long-running dev servers
- shell syntax requiring native shell semantics

Examples from the current implementation include:

- `tail -f`
- interactive editors
- `cargo run`
- `docker run` / `docker exec`
- `kubectl exec`
- Node/Python watch or interactive modes

Primary reference:

- [src/shell/policy.rs](/Users/jiatwork/Works/context/so-context/src/shell/policy.rs:1)

## Shared Policy Artifact

The policy must be shareable across:

- Rust hook code
- generated OpenCode TypeScript plugin code

This does not require one runtime language.

Acceptable sharing strategies:

- Rust source of truth plus generated JSON/TS constants
- host-specific entry points that delegate routing decisions into the shared Rust hook
- policy schema + Rust/TS interpreters
- generated matcher lists and generated policy tables

Unacceptable outcome:

- semantic policy drift between Rust hooks and OpenCode plugin logic

## Compatibility Constraints

The first migration must preserve the observable behavior of current host
integrations:

- same reroute intent
- same retry target tool
- same argument normalization semantics
- same pass-through cases for unsafe/complex inputs

The exact user-facing wording may evolve, but the policy result should not drift
silently.

## Verification Checklist

Before implementing `RoutingService`, confirm:

- a native read request produces the same retry args across supported hosts
- a regex-style search passes through across supported hosts
- a simple `git status` reroutes across supported hosts
- a long-running command like `tail -f` remains native across supported hosts
- `so-context` tool calls receive `_so_session_id` enrichment where available

## Related References

- [docs/context-runtime-architecture.md](/Users/jiatwork/Works/context/so-context/docs/context-runtime-architecture.md)
- [docs/host-capability-matrix.md](/Users/jiatwork/Works/context/so-context/docs/host-capability-matrix.md)
- [src/hook/pre_tool.rs](/Users/jiatwork/Works/context/so-context/src/hook/pre_tool.rs:32)
- [src/setup/so-context.ts](/Users/jiatwork/Works/context/so-context/src/setup/so-context.ts:1)
- [src/shell/policy.rs](/Users/jiatwork/Works/context/so-context/src/shell/policy.rs:1)

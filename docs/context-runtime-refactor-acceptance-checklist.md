# Context Runtime Refactor Acceptance Checklist

This checklist is for acceptance, not planning.

It answers one question:

Can we reasonably say the current `context runtime architecture` refactor is
done for phase 1, without pretending that deferred future capabilities are
already implemented?

Status snapshot:

- Date: 2026-06-13
- Branch: `refactor/context-runtime-architecture`

## Acceptance Rule

Mark this refactor accepted when:

- the runtime spine exists in code
- the supported hosts use the shared runtime contract
- shared policy no longer lives independently in each host integration
- setup/install paths are split per host
- compact/reset is modeled as session continuity rather than shell behavior
- tests and static checks pass

Do not block acceptance on explicitly deferred capabilities such as:

- full resume hydration
- subagent lifecycle support
- prompt transform / permission interception / post-tool review surfaces

## A. Runtime Spine

- [x] `AgentAdapter` exists as an explicit host boundary.
- [x] `SetupRenderer` exists as an explicit setup/install boundary.
- [x] `RoutingService` is the single source of truth for read/search/shell reroute decisions.
- [x] `SessionService` owns session identity normalization and compact/reset ID derivation.
- [x] No central orchestration layer exists that only forwards calls.

Current evidence:

- [src/host_adapter/agent_adapter.rs](/Users/jiatwork/Works/context/so-context/src/host_adapter/agent_adapter.rs:1)
- [src/setup/renderer.rs](/Users/jiatwork/Works/context/so-context/src/setup/renderer.rs:1)
- [src/routing_session/routing_service.rs](/Users/jiatwork/Works/context/so-context/src/routing_session/routing_service.rs:1)
- [src/routing_session/session_service.rs](/Users/jiatwork/Works/context/so-context/src/routing_session/session_service.rs:1)

## B. Shared Policy Ownership

- [x] Native read aliases are shared policy, not host-local copies.
- [x] Native search aliases are shared policy, not host-local copies.
- [x] Native shell routing policy is shared policy, not host-local copies.
- [x] Retry guidance is generated from a shared contract and rendered per host.
- [x] Host-specific tool naming is derived from host capability data.

Current evidence:

- [src/routing_session/host_metadata.rs](/Users/jiatwork/Works/context/so-context/src/routing_session/host_metadata.rs:1)
- [src/shell/policy.rs](/Users/jiatwork/Works/context/so-context/src/shell/policy.rs:1)
- [src/routing_session/types.rs](/Users/jiatwork/Works/context/so-context/src/routing_session/types.rs:1)
- [src/host_adapter/profile.rs](/Users/jiatwork/Works/context/so-context/src/host_adapter/profile.rs:1)

## C. Host Integration Split

- [x] Codex has an explicit host profile.
- [x] Claude has an explicit host profile.
- [x] OpenCode has an explicit host profile.
- [x] Codex setup is installed through a dedicated renderer path.
- [x] Claude setup is installed through a dedicated renderer path.
- [x] OpenCode setup is installed through a dedicated renderer path.
- [x] Hook/plugin binding helpers are shared where the host surface allows it.

Current evidence:

- [src/host_adapter/profile.rs](/Users/jiatwork/Works/context/so-context/src/host_adapter/profile.rs:41)
- [src/setup/codex.rs](/Users/jiatwork/Works/context/so-context/src/setup/codex.rs:1)
- [src/setup/claude.rs](/Users/jiatwork/Works/context/so-context/src/setup/claude.rs:1)
- [src/setup/opencode.rs](/Users/jiatwork/Works/context/so-context/src/setup/opencode.rs:1)
- [src/setup/hook_binding.rs](/Users/jiatwork/Works/context/so-context/src/setup/hook_binding.rs:1)

## D. OpenCode Convergence

- [x] OpenCode still uses its plugin execution path, but no longer owns an entirely separate retry naming contract.
- [x] OpenCode session identity paths are generated from shared host capability data.
- [x] OpenCode pre-tool routing decisions are delegated into the shared Rust hook via CLI.
- [x] OpenCode continues to support compact/reset through its plugin compact event.

Acceptance note:

OpenCode does not need to share the exact same host entry surface as Codex and
Claude to pass phase-1 acceptance. It only needs to share the same runtime
contract semantics, which phase 1 now does by delegating pre-tool decisions
from the plugin into the shared Rust hook.

Current evidence:

- [src/setup/opencode.rs](/Users/jiatwork/Works/context/so-context/src/setup/opencode.rs:158)
- [src/setup/so-context.ts](/Users/jiatwork/Works/context/so-context/src/setup/so-context.ts:1)

## E. Session Continuity

- [x] Compact/reset is handled as session continuity.
- [x] Compact/reset ID derivation is centralized.
- [x] Hosts without `connection_id` fall back explicitly rather than inventing fake resume semantics.
- [x] Compact/reset is not mixed with `so_shell` compression abstractions.

Current evidence:

- [src/routing_session/session_service.rs](/Users/jiatwork/Works/context/so-context/src/routing_session/session_service.rs:1)
- [src/hook/post_compact.rs](/Users/jiatwork/Works/context/so-context/src/hook/post_compact.rs:1)
- [docs/context-runtime-architecture.md](/Users/jiatwork/Works/context/so-context/docs/context-runtime-architecture.md:170)

## F. External Contract Stability

- [x] Existing MCP tool names remain unchanged for Codex/Claude.
- [x] Existing OpenCode flattened tool names remain unchanged.
- [x] `so_read`, `so_search`, `so_shell` contracts are preserved.
- [x] Setup / uninstall entrypoints remain available through CLI commands.

Current evidence:

- [src/main.rs](/Users/jiatwork/Works/context/so-context/src/main.rs:55)
- [src/setup/mod.rs](/Users/jiatwork/Works/context/so-context/src/setup/mod.rs:1)

## G. Verification

- [x] `cargo check --locked`
- [x] `cargo test --locked`
- [x] `cargo clippy --locked --all-targets -- -D warnings`

Latest known result:

- `cargo test --locked`: `275 passed; 0 failed`
- `cargo clippy --locked --all-targets -- -D warnings`: passed

## H. Deferred By Design

These are not acceptance failures for this refactor:

- [ ] Full resume hydration
- [ ] Subagent lifecycle support
- [ ] Session start / resume hooks
- [ ] Permission interception
- [ ] Post-tool review hooks
- [ ] Prompt transformation / prompt append

Interpretation:

- unchecked here means intentionally deferred, not accidentally incomplete

## Current Verdict

Phase-1 `context runtime architecture` refactor status:

- Runtime spine: accepted
- Shared policy ownership: accepted
- Host split: accepted
- OpenCode convergence: accepted for phase 1
- Session continuity: accepted for compact/reset scope
- Deferred future capabilities: not implemented by design

Overall verdict:

- The current refactor is complete for its intended phase-1 scope.
- What remains after this point is capability expansion, not unfinished
  refactor plumbing.

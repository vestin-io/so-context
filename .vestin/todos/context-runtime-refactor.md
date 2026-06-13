# Context Runtime Refactor TODOs

This is a migration checklist, not an implementation backlog explosion.

## Phase 0: Freeze The Direction

- [x] Confirm class/service-oriented architecture as the target model.
- [x] Separate host adapter, runtime, setup renderer, and shared-core
  integration boundaries.
- [x] Freeze the rule that compact/resume is not shell compression.
- [x] Freeze the rule that each host owns independent setup and injection.

## Phase 1: Introduce The Minimal Runtime Spine

- [x] Define the thin `AgentAdapter` contract.
- [x] Define the thin `SetupRenderer` contract.
- [x] Define `RoutingService` as the single source of truth for native tool
  aliases and reroute decisions.
- [x] Define `SessionService` ownership for session ID normalization and compact
  reset behavior.
- [x] Define a layered identity model for session / agent / connection IDs.
- [x] Define how shared routing policy is shared across Rust hooks and OpenCode
  plugin code.

Exit condition:

- the architecture can be represented in code without adding a pass-through
  orchestration layer

## Phase 2: Move Existing Behavior Behind The Spine

- [x] Move native read/search/shell interception policy behind `RoutingService`.
- [x] Remove duplicated matcher/policy knowledge from per-host setup logic.
- [x] Keep current MCP tool contracts unchanged.
- [x] Keep current installed hosts functioning with equivalent behavior.
- [x] Preserve host-specific tool namespace mapping while sharing policy.

Exit condition:

- host integrations become thin bindings instead of policy owners

## Phase 3: Split Per-Host Responsibilities Cleanly

- [x] Introduce explicit adapter responsibilities for OpenCode.
- [x] Introduce explicit adapter responsibilities for Claude.
- [x] Introduce explicit adapter responsibilities for Codex.
- [x] Introduce explicit setup renderer responsibilities per host.
- [x] Document how a fourth host would be added without copy-pasting policy.
- [x] Document optional host extension surfaces that are deferred in phase 1.

Exit condition:

- adding a new host is adapter/setup work, not runtime duplication

## Phase 4: Make Session Continuity First-Class

- [x] Specify compact/reset actions.
- [x] Define degraded continuity behavior for hosts without full resume support.
- [x] Bind cache reset behavior to session continuity rules, not ad hoc hooks.

Exit condition:

- compact/reset becomes an explicit session capability family

## Phase 5: Prepare Future Capabilities

- [x] Map repeat-read deduplication onto session/cache boundaries.
- [x] Map large-output FTS offloading onto tool-surface and search boundaries.
- [x] Map MCP tool description shrinking onto rule/tool-surface boundaries.
- [x] Map MCP stdio transparent proxy onto transport/tool-surface boundaries.

Exit condition:

- planned capabilities fit existing architecture without new top-level
  reshuffling

## Review Questions Before Any Code Refactor

- Is any host adapter still owning shared policy?
- Is any setup renderer still hard-coding matcher policy independently?
- Does any proposed module mix compact/resume and shell compression?
- Is there any central runtime layer that only forwards calls?
- Can a future host be added without editing shared core capability code?
- Are we preserving current external tool contracts for the first migration?

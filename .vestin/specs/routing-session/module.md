# Routing And Session Services Module

## Purpose / Responsibilities

This module defines the smallest shared service layer needed for the refactor.

It is responsible for:

- shared command routing decisions
- shared native alias classification
- normalized identity handling
- compact-reset continuity behavior

It is not responsible for:

- host config file manipulation
- graph/search implementation details
- speculative full resume orchestration

## Data Models

### Routing classification

Fields:

- external tool name or alias
- capability class
- normalized retry shape

### Session identity

Fields:

- runtime context ID
- host identifier
- host-native session identifier
- host-native agent identifier
- host-native connection identifier

### Routing decision

Fields:

- decision kind: allow / deny / rewrite / enrich
- rationale
- normalized retry or updated input payload

## Invariants

- routing classification MUST have one canonical source of truth
- compact-reset continuity MUST remain separate from shell compression
- the first migration MUST keep the call path minimal
- session, agent, and connection identity MUST remain distinguishable

## Interfaces

Inputs:

- normalized runtime events from adapters
- shared-core tool invocation requests

Outputs:

- routing decisions
- compact-reset actions
- normalized identity attribution

Error contract:

- unknown commands degrade to pass-through unless unsafe
- missing session identity degrades to best-effort attribution

## Failure Model

Potential failures:

- unclassified host-native tool aliases
- ambiguous session identity
- mismatch between routing decision and host lifecycle support

Degradation:

- pass through unknown commands when safe
- keep compact/reset behavior explicit and small when resume is unavailable

## Constraints

- preserve current public MCP tool contracts during the first migration
- support future repeat-read dedupe and fuller resume later without changing the
  top-level call chain
- support hosts that surface different identity layers

## Feature Index

- `command-interception` — [command-interception.md](./features/command-interception.md)
- `session-lifecycle` — [session-lifecycle.md](./features/session-lifecycle.md)
- `identity-model` — [identity-model.md](./features/identity-model.md)
- `compact-reset-contract` — [compact-reset-contract.md](./features/compact-reset-contract.md)

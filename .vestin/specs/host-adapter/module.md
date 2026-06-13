# Host Adapter Module

## Purpose / Responsibilities

The host adapter module defines the per-agent boundary between a concrete agent
host and the shared context runtime.

It is responsible for:

- declaring host lifecycle capabilities
- normalizing host events into runtime events
- extracting host identity fields
- translating host-native tool names into shared capability classes

It is not responsible for:

- shared interception policy
- shared session continuity rules
- setup file rendering

## Data Models

### Host capability profile

Fields:

- host identifier
- supported lifecycle hooks
- supported optional extension hooks
- supported instruction injection surfaces
- session / agent / connection identity sources
- compact/resume surface availability
- native tool alias set

Invariants:

- a capability profile must describe what the host can do, not what policy the
  runtime should choose

### Normalized runtime event

Fields:

- event kind
- host identifier
- normalized session identity
- normalized tool metadata
- raw payload reference

Persistence notes:

- event persistence is owned by runtime/core, not the adapter module

## Invariants

- adapters MUST translate host differences, not encode shared routing policy
- adapters MUST expose lifecycle capability gaps explicitly
- adapters MUST expose optional host extension surfaces explicitly
- one adapter MUST NOT depend on another adapter

## Interfaces

Inputs:

- host lifecycle payloads
- host tool invocation payloads
- shared runtime metadata requests

Outputs:

- normalized runtime events
- capability profile metadata

Error contract:

- malformed host payloads return normalization failure
- missing optional host capabilities degrade explicitly rather than implicitly

## Failure Model

Potential failures:

- host payload shape changes
- host session identifiers missing or unstable
- host-native tool alias drift

Degradation:

- pass through unknown host events when safe
- avoid fabricating unsupported lifecycle capability

Retryable:

- yes for setup/materialization mismatches after config repair

## Constraints

- must support current known hosts: OpenCode, Claude, Codex
- must allow future hosts without changing shared interception semantics
- must keep host-specific branching out of shared core

## Feature Index

- `capability-profile` — [capability-profile.md](./features/capability-profile.md)
- `lifecycle-binding` — [lifecycle-binding.md](./features/lifecycle-binding.md)

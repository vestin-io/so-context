# Shared Core Integration Module

## Purpose / Responsibilities

The shared core integration module defines how the refactored runtime continues
to consume existing core capabilities without pulling host-specific concerns
downward.

It is responsible for:

- binding runtime/tool-surface actions into watch, graph, cache, shell, and
  event capabilities
- keeping public tool surfaces stable during the first migration
- defining future extension hooks for non-MCP transport work

It is not responsible for:

- host lifecycle parsing
- setup rendering
- shared policy definition

## Data Models

### Tool surface contract

Fields:

- tool name
- request schema
- response schema
- compatibility guarantees

### Core capability binding

Fields:

- runtime action kind
- target shared-core subsystem
- compatibility notes

## Invariants

- first-stage refactor must preserve current MCP tool contracts
- shared-core integration must not branch by host when a runtime-level
  abstraction is sufficient

## Interfaces

Inputs:

- runtime actions
- tool invocation requests

Outputs:

- calls into watch/graph/cache/shell/events capabilities

Error contract:

- transport or contract failures surface at tool/runtime boundaries, not as host
  policy leaks

## Failure Model

Potential failures:

- mismatch between shared-service behavior and existing tool contracts
- transport-specific code leaking into core modules

Degradation:

- preserve existing external contracts while internal wiring evolves

## Constraints

- current README capabilities must still map cleanly into the new boundaries
- future transport work must not require re-splitting host adapters

## Feature Index

- `tool-surface-compatibility` — [tool-surface-compatibility.md](./features/tool-surface-compatibility.md)
- `future-capability-hooks` — [future-capability-hooks.md](./features/future-capability-hooks.md)

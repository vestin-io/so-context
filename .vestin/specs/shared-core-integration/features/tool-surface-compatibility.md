# Feature: Tool Surface Compatibility

## Feature Purpose

Keep the current external tool surface stable while moving internal behavior
behind the new runtime boundaries.

## Requirements

- The first migration stage MUST preserve the current MCP tool contract set.
- Shared-service boundary changes MUST be internal and MUST NOT require
  immediate host re-education on tool usage.
- Shared core integration MUST keep tool-surface compatibility separate from
  host setup concerns.
- Compatibility guarantees SHOULD be documented per tool surface during the
  migration.

## Verification

- Compare documented public tool set before and after refactor planning.
- Review architecture boundaries and confirm tool-surface compatibility lives in
  shared-core integration rather than host adapters.

## Failure / Edge Cases

- internal reroute logic changes while output schema must remain identical
- future non-MCP tool surfaces need the same runtime decisions

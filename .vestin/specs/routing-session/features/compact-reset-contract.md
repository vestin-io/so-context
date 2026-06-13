# Feature: Compact Reset Contract

## Feature Purpose

Define compact/reset as the first explicit session continuity contract.

## Requirements

- Compact/reset MUST be modeled as part of session lifecycle.
- The first migration MUST support compact/reset even if full resume is not yet
  implemented.
- Compact/reset behavior MUST integrate with cache invalidation rules.
- Future resume support SHOULD extend this contract rather than replace it.
- Compact/reset MUST remain separate from shell output compression semantics.

## Verification

- Review architecture boundaries and confirm compact/reset lives under session
  concerns.
- Review invariants and confirm shell compression is treated as a separate
  capability.

## Failure / Edge Cases

- host supports compaction hooks but no resume hydration
- compact event arrives without stable session identity

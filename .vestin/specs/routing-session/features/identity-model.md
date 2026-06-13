# Feature: Identity Model

## Feature Purpose

Preserve the identity layers exposed by different hosts so routing, attribution,
compaction, and future sub-agent support do not collapse onto one fake ID.

## Requirements

- The shared service layer MUST distinguish session identity, agent identity,
  and connection identity when hosts expose them.
- The normalized model MAY derive a runtime context identifier for internal use,
  but it MUST preserve the original identity layers.
- Hosts that expose only one identity layer MUST still map cleanly into the
  normalized model.
- Cache, event, and transport consumers MUST use the correct identity layer for
  their concern instead of reusing one arbitrary ID everywhere.

## Verification

- Architecture review confirms identity is modeled as layered, not singular.
- Feature review confirms compact-reset and attribution specs depend on this
  model.

## Data Model Impact

- extends the conceptual `RuntimeSessionContext` with layered host identities

## Failure / Edge Cases

- host exposes session and agent IDs but no stable connection ID
- host exposes connection-scoped IDs that outlive or underlive session scope
- future sub-agent hosts create nested agent identities

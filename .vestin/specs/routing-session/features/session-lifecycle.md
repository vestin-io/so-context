# Feature: Session Lifecycle

## Feature Purpose

Normalize session identity and attribution across hosts.

## Requirements

- The shared service layer MUST define one normalized session identity model.
- Session attribution MUST work across current known hosts even when their
  native identifiers differ.
- Session lifecycle MUST build on the layered identity model instead of
  replacing it.
- Missing or partial host session identifiers MUST degrade to best-effort
  attribution instead of forcing host-specific hacks into shared core.

## Verification

- Module review confirms session identity is shared-service owned rather than
  spread across host integrations.

## Data Model Impact

- introduces the conceptual `RuntimeSessionContext`

## Failure / Edge Cases

- host provides `session_id` and `agent_id` with different stability semantics
- connection-scoped identifiers differ from agent-session identifiers

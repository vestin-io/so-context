# Feature: Lifecycle Binding

## Feature Purpose

Define how each host adapter binds host lifecycle/tool events into the shared
runtime event model.

## Requirements

- Each adapter MUST translate host lifecycle events into normalized runtime
  events.
- Lifecycle binding MUST include tool invocation, session continuity, and setup
  relevant events where the host supports them.
- When a host lacks a lifecycle surface, the adapter MUST degrade explicitly and
  MUST NOT invent unsupported semantics.
- Lifecycle binding MUST keep host payload parsing inside the adapter boundary.

## Verification

- Architecture review confirms event normalization is adapter-owned.
- Module review confirms runtime services receive normalized events rather than
  raw host payload shapes.

## Failure / Edge Cases

- Hook payload shape changes between host versions.
- A host exposes tool events but not compact/resume events.
- Host emits insufficient metadata to attribute session continuity perfectly.

# Feature: Instruction Injection

## Feature Purpose

Define how shared `so-context` usage guidance is rendered into each host's
instruction surface without turning instruction files into policy sources.

## Requirements

- The renderer MUST inject shared instruction guidance into each supported
  host's instruction surface when such a surface exists.
- Shared instruction fragments MUST originate from a shared runtime-owned rule
  source.
- Host-specific instruction formatting MAY differ, but the behavioral guidance
  MUST stay semantically consistent across hosts.
- Instruction injection MUST remain independent from command classification
  logic.

## Verification

- Spec review confirms instruction content is shared-source, host-rendered.
- Architecture review confirms instruction injection is distinct from setup of
  hook/plugin mechanisms.

## Failure / Edge Cases

- host supports no separate instruction file
- host instruction file already contains user-managed custom guidance

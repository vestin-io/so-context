# Feature: Host Extension Surface

## Feature Purpose

Acknowledge host lifecycle and interception surfaces that exist today but are
not required in phase 1, so the architecture stays compatible with future
expansion.

## Requirements

- The architecture MUST represent optional host extension surfaces as capability
  declarations on adapters.
- Optional extension surfaces MUST include room for session start/resume,
  sub-agent lifecycle, permission interception, post-tool review, prompt
  append/transform, and pre-compact customization when hosts expose them.
- The first migration MUST NOT require implementing all optional extension
  surfaces.
- Future support for an extension surface SHOULD attach to existing adapter and
  routing/session boundaries before introducing a new top-level module.

## Verification

- Architecture review confirms optional host extensions are explicitly named.
- Spec review confirms phase-1 scope stays small while extension surfaces remain
  representable.

## Failure / Edge Cases

- a host exposes a richer prompt lifecycle than current adapters model
- one host exposes permission interception while another does not

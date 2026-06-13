# Feature: Future Capability Hooks

## Feature Purpose

Reserve explicit integration points for planned capabilities so future work does
not require another architectural reset.

## Requirements

- The architecture MUST provide stable extension points for repeat-read
  deduplication.
- The architecture MUST provide stable extension points for large-output FTS
  offloading.
- The architecture MUST provide stable extension points for MCP tool
  description shrinking.
- The architecture MUST provide stable extension points for MCP stdio
  transparent proxy work.
- Future capability hooks SHOULD reuse existing runtime/service boundaries
  before introducing new module families.
- Future capability hooks MUST coexist with host-specific optional extension
  surfaces without forcing policy duplication.

## Verification

- Architecture review confirms each planned capability maps to an existing
  target module boundary.
- No planned capability requires host-policy duplication in the architecture
  spec.

## Failure / Edge Cases

- a future capability spans runtime and transport concerns
- a future host adds a lifecycle surface that existing adapters do not expose

# Feature: Host Config Rendering

## Feature Purpose

Render per-host MCP/hook/plugin setup using shared routing metadata and
host-specific file formats.

## Requirements

- Each supported host MUST have its own setup renderer behavior.
- Setup rendering MUST register the shared runtime entry points needed by that
  host, including MCP bridge and lifecycle hooks where applicable.
- Setup rendering MUST source matcher/capability metadata from shared routing
  definitions.
- Uninstall MUST remove `so-context` owned artifacts without deleting unrelated
  user configuration.

## Verification

- Architecture review confirms renderer inputs include shared routing metadata.
- Setup spec review confirms each host has independent rendering behavior.

## Failure / Edge Cases

- host config already contains user hooks in the same matcher group
- host config schema differs across versions
- host uses plugin files instead of config-only setup

# Setup Renderer Module

## Purpose / Responsibilities

The setup renderer module owns install/uninstall behavior for supported hosts.

It is responsible for:

- rendering host config changes
- registering MCP entry points, hooks, and plugins
- wiring instruction files into host-supported surfaces

It is not responsible for:

- shared routing policy
- session lifecycle semantics
- shared command classification logic

## Data Models

### Setup target

Fields:

- host identifier
- config file paths
- plugin/template outputs
- instruction injection targets

### Rendered host artifact

Fields:

- artifact kind
- destination path
- content source
- uninstall cleanup rule

## Invariants

- setup renderers MUST consume shared runtime metadata rather than copy policy
- each supported host MUST have independent setup rendering behavior
- uninstall behavior MUST be scoped to `so-context` owned artifacts

## Interfaces

Inputs:

- host capability profile
- shared routing metadata
- shared instruction fragments

Outputs:

- host config updates
- plugin/instruction artifacts

Error contract:

- partial setup failures must be reported per host
- failure in one host renderer must not imply silent success for another

## Failure Model

Potential failures:

- invalid host config syntax
- changed host config schema
- missing write permissions

Degradation:

- report host-local warnings without corrupting unrelated host configs

## Constraints

- support current hosts with independent setup surfaces
- keep uninstall reversible

## Feature Index

- `host-config-rendering` — [host-config-rendering.md](./features/host-config-rendering.md)
- `instruction-injection` — [instruction-injection.md](./features/instruction-injection.md)

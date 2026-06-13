# Context Runtime Refactor Architecture

## Working Assumptions

This architecture document is based on:

- the current repository structure
- the current `README.md` capability list
- the branch discussion about future multi-agent and multi-host support

The repository does not currently contain `.vestin/discovery.md` or
`.vestin/ideate.md`, so the assumptions below are being frozen directly from the
current conversation:

- `so-context` must support more agent hosts over time
- each host must own independent setup and injection behavior
- shared context policy must not be duplicated per host
- compact/resume must become first-class
- backward compatibility with current MCP tools matters

## Architecture Diagram

```text
Agent Host
  -> AgentAdapter
      -> RoutingService
      -> SessionService
      -> ToolSurface
          -> Shared Core
              -> Watch Registry
              -> Graph / Search
              -> Cache
              -> Shell Compression
              -> Event / Session Store

Setup command
  -> SetupRenderer per host
      -> host config files / plugins / instruction files
```

## Components

### 1. Host Adapter Layer

Responsibilities:

- bind host lifecycle events into runtime events
- extract host identity fields
- surface host capabilities
- translate host-specific tool names into shared command classes

Must not:

- define interception policy rules
- reimplement shared retry argument normalization
- own shared session continuity behavior

Examples:

- `OpenCodeAdapter`
- `ClaudeAdapter`
- `CodexAdapter`
- future adapters for additional agent hosts

### 2. Context Runtime Layer

Responsibilities:

- keep shared behavior consistent across hosts through a minimal service set
- avoid duplicated routing/session logic

Must not:

- know host config file formats
- know plugin file templates
- depend on MCP transport specifics

### 3. Minimal Shared Services

#### `RoutingService`

Responsibilities:

- evaluate native tool usage
- decide pass-through vs reroute
- own the canonical alias/capability classification needed for routing
- produce retry payload normalization
- keep user-facing guidance stable

Implementation note:

- shared routing behavior may need a shared policy artifact rather than a single
  shared executable path, because some hosts evaluate routing in Rust hooks and
  others in TypeScript plugins

#### `SessionService`

Responsibilities:

- normalize identity layers
- attribute events to agent sessions
- own compact/reset state transitions for the first migration
- coordinate cache invalidation tied to compaction

Identity layers to preserve:

- session identity
- agent or sub-agent identity
- connection identity

### 4. Setup Renderer Layer

Responsibilities:

- render install/uninstall operations for each host
- own file formats and config patching behavior
- register MCP bridge, hooks, plugins, and instruction files

Must not:

- own shared runtime policy
- define session lifecycle rules
- classify commands independently

### 5. Shared Core Integration Layer

Responsibilities:

- expose existing graph, cache, shell, watch, and event capabilities
- remain reusable from MCP and future non-MCP surfaces
- isolate transport-specific behavior from core capability logic

Must not:

- become aware of host-specific setup rules

## Module Boundaries

### Module: `host-adapter`

Belongs here:

- host capability declarations
- host lifecycle bindings
- session ID extraction rules
- host-native tool alias mapping inputs

Must not contain:

- shell/read/search routing policy
- cache reset policy
- shared tool contract rules

### Module: `routing-session`

Belongs here:

- shared routing decisions
- shared identity/session handling
- compact/reset lifecycle decisions

Must not contain:

- host config file edit logic
- storage implementation details of graph/search

### Module: `setup-renderer`

Belongs here:

- install/uninstall rendering
- host config manipulation
- plugin/template emission
- instruction file wiring

Must not contain:

- runtime policy duplication
- command alias lists copied from runtime

### Module: `shared-core-integration`

Belongs here:

- bridge from runtime/tool surface into existing watch/graph/cache/shell/events
- future transport expansion points

Must not contain:

- host-specific branching

## Data Flow

### Tool invocation flow

1. Host emits a tool/lifecycle event.
2. `AgentAdapter` converts it into a normalized runtime event.
3. `RoutingService` decides allow/deny/rewrite/enrich.
5. If needed, shared tool surfaces invoke shared core capabilities.
6. `SessionService` records attribution and continuity side effects.

### Setup flow

1. CLI setup command selects supported hosts.
2. Each host-specific `SetupRenderer` patches the target config surface.
3. Shared instruction fragments come from a shared template/module.
4. Shared matcher/capability metadata comes from routing metadata.

### Compact/resume flow

1. Host emits compact event or equivalent lifecycle boundary.
2. `AgentAdapter` passes normalized compact event to `SessionService`.
3. `SessionService` stores session continuity state and resets stale cache edges.
4. Resume-capable hosts hydrate future session state through the same service.

### Identity flow

1. Host emits one or more identity fields.
2. `AgentAdapter` extracts session, agent, and connection identity when present.
3. `SessionService` preserves those layers and derives runtime attribution from
   them.
4. Shared core integrations consume the appropriate identity layer for cache,
   watch, event, or transport concerns.

## Control Flow

The dependency direction must stay:

`AgentAdapter` -> `RoutingService` / `SessionService` -> shared core

and

`SetupRenderer` -> shared runtime metadata

but never:

- shared core -> host adapter
- runtime services -> host config format code
- one host adapter -> another host adapter

## Interface Outline

Conceptual only:

- `AgentAdapter`
  - declares host capabilities
  - normalizes host events into runtime events
- `RoutingService`
  - computes routing decisions and retry payloads
- `SessionService`
  - manages session identity and continuity transitions
- `SetupRenderer`
  - applies host config changes using shared runtime metadata

## Technology Decisions

### Decision: class/service-oriented Rust structure

Rationale:

- matches the requested mental model
- makes host boundaries explicit
- avoids an unstructured pile of free functions

Alternative rejected:

- purely functional module sprawl with no explicit service ownership

### Decision: composition over inheritance

Rationale:

- Rust favors owned composition
- keeps services small and testable
- avoids a deep OO hierarchy

Alternative rejected:

- inheritance-like hierarchy through overly broad traits

### Decision: preserve current MCP tools during first migration

Rationale:

- reduces rollout risk
- protects existing users and installed hosts
- allows internal architecture cleanup first

Alternative rejected:

- changing tool contracts while also changing runtime boundaries

### Decision: compact/resume as session concern, not shell concern

Rationale:

- different lifecycle and persistence semantics
- avoids coupling output compression with session continuity

Alternative rejected:

- treating all token-saving features as one generic optimization layer

### Decision: avoid a central orchestration object in the first migration

Rationale:

- keeps the call chain short
- avoids a pass-through `ContextRuntime` layer
- reduces framework-like abstraction before it is needed

Alternative rejected:

- introducing a top-level runtime coordinator that mostly forwards calls

### Decision: preserve multiple identity layers instead of collapsing to one ID

Rationale:

- current hosts already expose more than one useful identity
- compaction, attribution, sub-agents, and MCP connection handling do not all
  share the same lifecycle
- keeps transport concerns from leaking into fake session IDs

Alternative rejected:

- collapsing session, agent, and connection identity into one opaque string

## NFR Mapping

### Extensibility

Constraint:

- new hosts and new host-native commands must be added without copying policy

Tactic:

- shared routing policy artifact
- per-host adapter + setup renderer split

### Backward compatibility

Constraint:

- current MCP tools and existing setup installs should keep working during refactor

Tactic:

- preserve external tool contracts first
- move internals behind new runtime classes before behavioral expansion

### Simplicity

Constraint:

- avoid over-abstracting early

Tactic:

- only four durable module families
- composition-first services
- no speculative feature modules until a capability is real

### Observability

Constraint:

- future compact/resume and routing behavior must remain debuggable

Tactic:

- keep session and interception concerns explicit
- record decisions through existing event infrastructure where appropriate

### Compatibility

Constraint:

- current Codex, Claude, and OpenCode integration styles differ in code path and
  lifecycle surface

Tactic:

- allow policy sharing through artifacts/metadata, not only shared executable
  code
- declare optional host extension surfaces instead of assuming all hosts look
  the same

## Risks And Mitigations

### Risk: adapter becomes a god object

Mitigation:

- enforce adapter boundary as translation only
- push policy into runtime services

### Risk: setup renderers drift from runtime behavior

Mitigation:

- setup must consume matcher/capability metadata from shared runtime sources

### Risk: compact/resume scope expands too early

Mitigation:

- specify compact/reset/resume contracts first
- delay richer resume payload design until hook boundaries are stable

### Risk: future transport work leaks into host adapters

Mitigation:

- keep MCP/proxy work inside tool-surface or shared-core integration layer

### Risk: "shared routing" turns into duplicated Rust + TypeScript logic again

Mitigation:

- define a shared policy artifact boundary early
- allow generated metadata/constants as a first-class sharing mechanism

### Risk: session identity is over-collapsed and later blocks sub-agent support

Mitigation:

- preserve session, agent, and connection identity separately from the start

## Recommended Next Artifact

Write module-level specs for:

- `host-adapter`
- `context-runtime`
- `setup-renderer`
- `shared-core-integration`

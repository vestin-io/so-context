# Feature: Host Capability Profile

## Feature Purpose

Define a stable per-host capability declaration so the shared runtime can reason
about what each host supports without hard-coding that knowledge in multiple
places.

## Requirements

- Each supported host MUST expose one capability profile.
- A capability profile MUST describe lifecycle hooks, instruction injection
  surfaces, identity sources, compact/resume support, optional extension
  surfaces, and native tool alias classes.
- Capability profiles MUST separate host capability description from runtime
  policy decisions.
- Adding a new host SHOULD require adding a new capability profile before any
  runtime behavior is implemented.

## Verification

- Architecture review confirms every supported host maps to one capability
  profile concept.
- Spec review confirms policy decisions are absent from capability-profile
  fields.

## Data Model Impact

- introduces the conceptual `HostCapabilityProfile` entity

## Failure / Edge Cases

- Host lacks explicit compact/resume support: profile records absence instead of
  emulating support implicitly.
- Host exposes multiple session identifiers: profile must document precedence.

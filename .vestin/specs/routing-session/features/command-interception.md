# Feature: Command Interception

## Feature Purpose

Provide one shared routing policy for native read/search/shell activity.

## Requirements

- Shared routing MUST classify commands through one canonical alias/capability
  mapping.
- Shared routing MUST support allow, deny-with-retry-guidance, and
  rewrite/enrich outcomes.
- Retry guidance MUST normalize arguments into the shared tool surface contract.
- Host adapters and setup renderers MUST NOT define independent interception
  policy.
- Unknown or advanced native command shapes SHOULD pass through when the system
  cannot safely normalize them.

## Verification

- Architecture review confirms one shared routing source is referenced by both
  runtime and setup artifacts.
- Feature review confirms denial/rewrite outcomes remain shared-service owned.

## Failure / Edge Cases

- regex or semantically rich native search should not be naively downgraded
- complex shell syntax should remain native
- future host-specific aliases should map into existing capability classes

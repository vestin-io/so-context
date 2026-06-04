# Shell Audit

Architecture overview: [README.md](README.md).

The formal shell audit command is:

```bash
bash scripts/shell_audit.sh
bash scripts/shell_audit.sh --suite core
bash scripts/shell_audit.sh --suite extended
```

This audit does not compare against RTK.
It runs a fixed suite of native commands and `so-context shell` commands, then writes a JSON report to:

```text
.so-context/audits/shell-audit-latest.json
```

Use this when you want a repeatable quality gate for shell output behavior.

Suites:

- `core`
  Stable representative coverage for the main families.
- `extended`
  Broader one-case-per-pattern coverage where fixtures are available.
  This is the default.

The report contains:

- command under test
- working directory
- classified family
- render mode
- raw bytes
- rendered bytes
- saved percent
- native exit code
- shell exit code
- full captured native and shell outputs
- suspicious reasons

The report marks each case as one of:

- `ok`
- `suspicious`
- `skipped`

RTK compare scripts remain optional reference tooling only.
They are not part of the formal audit path.

# Contributing to so-context

Thanks for your interest in contributing to `so-context`.

## Before you start

- Open an issue or start a discussion before large changes.
- Keep pull requests focused. Small, reviewable changes land faster.
- Include tests or update existing tests when behavior changes.

## Development setup

```sh
cargo build
cargo check
cargo test
```

Useful local commands:

```sh
so-context daemon
so-context setup
so-context metrics
```

## Pull request checklist

- The change is scoped and explained clearly.
- `cargo check` passes.
- `cargo test` passes.
- Documentation is updated when user-facing behavior changes.
- New config, hooks, or release behavior is called out in the PR description.

## Style

- Prefer simple, explicit Rust over clever abstractions.
- Preserve existing project structure and naming.
- Add comments only when they explain intent that is not obvious from the code.

## Reporting bugs

Please include:

- What you expected to happen
- What actually happened
- Reproduction steps
- Relevant logs, screenshots, or session snippets
- Your OS, shell, and agent environment when relevant

## License

By contributing to this repository, you agree that your contributions will be
licensed under the MIT License that applies to this project.

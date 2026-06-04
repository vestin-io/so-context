# Shell Support Matrix

Architecture and data flow: [README.md](README.md).

This file is the working reference for shell compression support in `so-context`.

Update it whenever we:

- add a new command family or subcommand
- change the default compression behavior for an existing pattern
- add or remove pattern-level tests

## Scope

Current shell entrypoint:

```sh
so-context shell -- <command> [args...]
```

Related flags:

- `--full`: bypass compression and print raw output

## Source Of Truth

Classifier and dispatch:

- [src/shell/patterns/mod.rs](../../src/shell/patterns/mod.rs)

Implementations:

- [src/shell/patterns/git/mod.rs](../../src/shell/patterns/git/mod.rs)
- [src/shell/patterns/docker/mod.rs](../../src/shell/patterns/docker/mod.rs)
- [src/shell/patterns/node/mod.rs](../../src/shell/patterns/node/mod.rs)
- [src/shell/patterns/rust/mod.rs](../../src/shell/patterns/rust/mod.rs)
- [src/shell/patterns/gh/mod.rs](../../src/shell/patterns/gh/mod.rs)
- [src/shell/patterns/k8s/mod.rs](../../src/shell/patterns/k8s/mod.rs)
- [src/shell/patterns/build/mod.rs](../../src/shell/patterns/build/mod.rs)
- [src/shell/patterns/generic/mod.rs](../../src/shell/patterns/generic/mod.rs)

## Status Legend

- `Supported`: command is classified and has a dedicated summarizer
- `Tested`: command has at least one pattern-level test
- `Raw only`: no dedicated summarizer; use `--full` for fidelity

## Git


| Command        | Pattern        | Status    | Tested | Notes                                               |
| -------------- | -------------- | --------- | ------ | --------------------------------------------------- |
| `git status`   | `git.status`   | Supported | Yes    | Handles porcelain and human-readable status output  |
| `git diff`     | `git.diff`     | Supported | Yes    | Default output is compacted file-level diff summary |
| `git log`      | `git.log`      | Supported | Yes    | Summarizes commit list                              |
| `git branch`   | `git.branch`   | Supported | Yes    | Summarizes branches and current branch              |
| `git remote`   | `git.remote`   | Supported | Yes    | Summarizes remotes                                  |
| `git show`     | `git.show`     | Supported | Yes    | Summarizes show output                              |
| `git fetch`    | `git.fetch`    | Supported | Yes    | Summarizes transport output                         |
| `git pull`     | `git.pull`     | Supported | Yes    | Summarizes transport output                         |
| `git push`     | `git.push`     | Supported | Yes    | Summarizes transport output                         |
| `git checkout` | `git.checkout` | Supported | Yes    | Summarizes branch/file transition                   |
| `git switch`   | `git.switch`   | Supported | Yes    | Summarizes branch transition                        |
| `git commit`   | `git.commit`   | Supported | Yes    | Summarizes commit result                            |
| `git add`      | `git.add`      | Supported | Yes    | Summarizes staged path count                        |
| `git clone`    | `git.clone`    | Supported | Yes    | Summarizes clone target and progress result         |
| `git merge`    | `git.merge`    | Supported | Yes    | Summarizes merge stats or conflicts                 |
| `git tag`      | `git.tag`      | Supported | Yes    | Summarizes tag list or create result                |
| `git reset`    | `git.reset`    | Supported | Yes    | Summarizes unstaged file count after reset          |
| `git stash`    | `git.stash`    | Supported | Yes    | Summarizes stash save/list/show/apply flows         |


## Docker


| Command             | Pattern          | Status    | Tested | Notes                                 |
| ------------------- | ---------------- | --------- | ------ | ------------------------------------- |
| `docker ps`         | `docker.ps`      | Supported | Yes    | Table summary with sample names       |
| `docker images`     | `docker.images`  | Supported | Yes    | Image table summary                   |
| `docker compose ps` | `docker.compose` | Supported | Yes    | Compose state summary                 |
| `docker compose`    | `docker.compose` | Supported | Yes    | Generic compose summary fallback      |
| `docker logs`       | `docker.logs`    | Supported | Yes    | Stream/log preview summary            |
| `docker build`      | `docker.build`   | Supported | Yes    | Build progress summary                |
| `docker inspect`    | `docker.inspect` | Supported | Yes    | JSON/object summary                   |
| `docker pull`       | `docker.pull`    | Supported | Yes    | Pull progress summary                 |
| `docker-compose`    | `docker.compose` | Supported | Yes    | Legacy binary maps to compose pattern |


## Node.js


| Command        | Pattern     | Status    | Tested | Notes                                                 |
| -------------- | ----------- | --------- | ------ | ----------------------------------------------------- |
| `npm`          | `node.npm`  | Supported | Yes    | Strips lifecycle boilerplate, notices, and warnings   |
| `pnpm`         | `node.pnpm` | Supported | Yes    | Filters progress noise and keeps compact result lines |
| `yarn`         | `node.yarn` | Supported | Yes    | Filters step progress and keeps compact result lines  |
| `bun`          | `node.bun`  | Supported | Yes    | Keeps concise install/build result lines              |
| `npx` / `bunx` | `node.npx`  | Supported | Yes    | Keeps tool output with minimal filtering              |


## Rust


| Command         | Pattern              | Status    | Tested | Notes                                                             |
| --------------- | -------------------- | --------- | ------ | ----------------------------------------------------------------- |
| `cargo build`   | `rust.cargo-build`   | Supported | Yes    | Counts actionable errors/warnings; keeps success/install lines    |
| `cargo test`    | `rust.cargo-test`    | Supported | Yes    | Compacts `test result:` lines and failure names                   |
| `cargo clippy`  | `rust.cargo-clippy`  | Supported | Yes    | `No issues found` on clean runs; otherwise counts errors/warnings |
| `cargo check`   | `rust.cargo-check`   | Supported | Yes    | Counts actionable errors/warnings; keeps success lines            |
| `cargo install` | `rust.cargo-install` | Supported | Yes    | Keeps installed package line and hides compile noise              |
| `cargo nextest` | `rust.cargo-nextest` | Supported | Yes    | Uses the same compact test-result contract as `cargo test`        |


## GitHub CLI


| Command    | Pattern    | Status    | Tested | Notes                             |
| ---------- | ---------- | --------- | ------ | --------------------------------- |
| `gh pr`    | `gh.pr`    | Supported | Yes    | Compact PR list/view summaries    |
| `gh issue` | `gh.issue` | Supported | Yes    | Compact issue list/view summaries |
| `gh run`   | `gh.run`   | Supported | Yes    | Compact workflow run summaries    |


## Kubernetes


| Command                | Pattern                | Status    | Tested | Notes                                                     |
| ---------------------- | ---------------------- | --------- | ------ | --------------------------------------------------------- |
| `kubectl get pods`     | `k8s.kubectl-pods`     | Supported | Yes    | Summarizes running/pending/failed pods and restart counts |
| `kubectl get services` | `k8s.kubectl-services` | Supported | Yes    | Compact service list with type and ports                  |
| `kubectl logs`         | `k8s.kubectl-logs`     | Supported | Yes    | Prefers error lines; otherwise shows short log sample     |
| `kubectl describe`     | `k8s.kubectl-describe` | Supported | Yes    | Keeps key describe fields like name, status, image        |
| `kubectl apply`        | `k8s.kubectl-apply`    | Supported | Yes    | Counts created/configured/unchanged/deleted resources     |


## Build Tools


| Command              | Pattern                | Status    | Tested | Notes                                                |
| -------------------- | ---------------------- | --------- | ------ | ---------------------------------------------------- |
| `tsc`                | `build.tsc`            | Supported | Yes    | `TypeScript: N errors in M files` contract           |
| `next build`         | `build.next`           | Supported | Yes    | Keeps build success line and route/dist evidence     |
| `vite build`         | `build.vite`           | Supported | Yes    | Keeps build success line and dist evidence           |
| `make`               | `build.make`           | Supported | Yes    | Keeps actionable summary/error lines                 |
| `gradle` / `gradlew` | `build.gradle`         | Supported | Yes    | Keeps `BUILD SUCCESSFUL/FAILED` and task/error lines |
| `mvn` / `mvnw`       | `build.maven`          | Supported | Yes    | Keeps build success/failure headline                 |
| `dotnet build`       | `build.dotnet-build`   | Supported | Yes    | Keeps build success/failure headline                 |
| `dotnet test`        | `build.dotnet-test`    | Supported | Yes    | Keeps passed/failed test summary                     |
| `dotnet restore`     | `build.dotnet-restore` | Supported | Yes    | Keeps restore completion headline                    |
| `dotnet format`      | `build.dotnet-format`  | Supported | Yes    | Keeps formatting result headline                     |
| `cmake`              | `build.cmake`          | Supported | Yes    | Keeps configure/generate/build completion summary    |


## Generic


| Command | Pattern        | Status    | Tested | Notes                                              |
| ------- | -------------- | --------- | ------ | -------------------------------------------------- |
| `ls`    | `generic.ls`   | Supported | Yes    | Counts entries and infers dirs/files when possible |
| `find`  | `generic.find` | Supported | Yes    | Path list summary                                  |
| `rg`    | `generic.rg`   | Supported | Yes    | Groups hits by file                                |
| `grep`  | `generic.grep` | Supported | Yes    | Groups hits by file                                |
| `curl`  | `generic.curl` | Supported | Yes    | Response/body summary                              |
| `wget`  | `generic.wget` | Supported | Yes    | Response/body summary                              |
| `env`   | `generic.env`  | Supported | Yes    | Lists variable names, not full values              |
| `cat`   | `generic.cat`  | Supported | Yes    | File excerpt summary                               |
| `head`  | `generic.head` | Supported | Yes    | File excerpt summary                               |
| `tail`  | `generic.tail` | Supported | Yes    | File excerpt summary                               |


## Fallback


| Case             | Pattern   | Status    | Notes                                                   |
| ---------------- | --------- | --------- | ------------------------------------------------------- |
| Unknown commands | `unknown` | Supported | Generic fallback summary; use `--full` for raw fidelity |


## Current Gaps

- The new Node.js / Rust / GitHub CLI / Kubernetes / build-tool families follow `rtk`-style output contracts, but they still only use post-exec compression; we do not yet inject command-specific JSON or output flags before execution
- Shell execution uses bounded buffered capture (10 MiB per stream) plus a 30s wall-clock timeout by default, but it is still not true streaming summarization
- We do not yet expose this matrix through CLI output
- We do not yet distinguish `experimental` vs `stable` patterns

## Update Checklist

When adding a new shell pattern:

1. Add classifier mapping in `src/shell/patterns/mod.rs`
2. Add or update summarizer in the relevant family file
3. Add at least one pattern-level test
4. Update this matrix

# `so_shell` Followed By `so_shell_output`

This note captures two kinds of `so_shell -> so_shell_output` cases from
session `rollout-2026-06-05T09-37-19-019e9491-bd69-75a0-9d3e-1af36063232a`:

1. cases where the initial `so_shell` result was already
   `output_mode: raw_fallback`
2. cases where the initial `so_shell` result was actually compressed and the
   agent later fetched full output

## Scope

- Session file:
  `/Users/jiatwork/.codex/sessions/2026/06/05/rollout-2026-06-05T09-37-19-019e9491-bd69-75a0-9d3e-1af36063232a.jsonl`
- Question answered here:
  Which `so_shell_output` follow-ups were effectively redundant, and which
  ones were caused by compressed output lacking needed detail?

## Method

For each `so_shell_output` call in the session:

1. Find the preceding `mcp_tool_call_end` event for the matching `run_id`.
2. Split cases by the initial `so_shell` `output_mode`.
3. For `raw_fallback` cases, compare the initial `so_shell` text payload
   against the later `so_shell_output` text payload.
4. For `compressed` cases, record the command shape and what kind of detail
   the compressed summary did not preserve well enough.

## Overview

- `29` total `so_shell_output` follow-ups in this session
- `15` followed `raw_fallback`
- `14` followed `compressed`

## Part 1: `raw_fallback` Cases

### Result

- `15` cases matched `raw_fallback -> so_shell_output`.
- `15 / 15` had exactly identical text payloads.
- `15 / 15` were also identical after trimming whitespace.

This means these cases were not "summary too short" incidents. The initial
`so_shell` content was already the same text later returned by
`so_shell_output`.

### Category Summary

| Category | Count |
| --- | ---: |
| `grep/search output` | 4 |
| `script output` | 3 |
| `git status` | 2 |
| `session/file path listing` | 2 |
| `db schema` | 1 |
| `repo state composite` | 1 |
| `port/process inspection` | 1 |
| `other` | 1 |

### Cases

| Run ID | Category | Command | CWD | Exit | JSONL lines | Text identical | Preview |
| --- | --- | --- | --- | ---: | --- | --- | --- |
| `shell-85512-1780627249536769000-105` | `git status` | `git status --short` | `/Users/jiatwork/Works/context/so-context` | 0 | `186 -> 195` | `yes` | ` M src/hook/pre_tool.rs` |
| `shell-85512-1780627427725474000-110` | `db schema` | `sqlite3 /Users/jiatwork/.local/share/so-context/events.db .schema` | `/Users/jiatwork/Works/context` | 0 | `268 -> 276` | `yes` | `CREATE TABLE events (` |
| `shell-85512-1780627427769051000-111` | `session/file path listing` | `sh -c 'find /Users/jiatwork/.codex/sessions -maxdepth 2 -type f \| head -40'` | `/Users/jiatwork/Works/context` | 0 | `270 -> 277` | `yes` | empty output |
| `shell-85512-1780630905307119000-114` | `repo state composite` | `sh -c 'git rev-parse --show-toplevel && git branch --show-current && git status --short'` | `/Users/jiatwork/Works/context` | 128 | `421 -> 431` | `yes` | `fatal: not a git repository` |
| `shell-85512-1780630966421858000-122` | `session/file path listing` | `sh -c 'find /Users/jiatwork/Works/context/so-context -maxdepth 2 ... \| sort \| head -200'` | `/Users/jiatwork/Works/context` | 0 | `482 -> 497` | `yes` | `scripts/install.sh` |
| `shell-85512-1780631343857791000-133` | `script output` | `python3 -c '<session usage script>'` | `/Users/jiatwork/Works/context` | 0 | `569 -> 576` | `yes` | `rollout-2026-06-05T09-37-19-... yes so_read,so_search...` |
| `shell-85512-1780632062600451000-135` | `grep/search output` | `rg -n '\\.agent\\s*=|agent: None|pub agent' /Users/jiatwork/Works/context/so-context/src --glob !target` | `/Users/jiatwork/Works/context` | 0 | `681 -> 687` | `yes` | `src/shell/telemetry.rs:18: pub agent: Option<String>` |
| `shell-85512-1780632422420718000-140` | `grep/search output` | `rg -n 'fn resolve_project_path_arg|fn infer_connection_project_for_path|...' /Users/jiatwork/Works/context/so-context-main-session-sql/src/mcp --glob !target` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 0 | `753 -> 757` | `yes` | `src/mcp/tools/context.rs:38:pub(crate) fn resolve_project_path_arg(` |
| `shell-85512-1780635452617582000-159` | `git status` | `git status --short` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 0 | `1035 -> 1043` | `yes` | ` M src/hook/pre_tool.rs` |
| `shell-85512-1780636232130079000-167` | `port/process inspection` | `lsof -nP -iTCP:4173 -sTCP:LISTEN` | none | 0 | `1209 -> 1217` | `yes` | `node ... TCP 127.0.0.1:4173 (LISTEN)` |
| `shell-85512-1780636232200826000-168` | `other` | `git -C /Users/jiatwork/Works/context/so-context-main-session-sql status --short session-viewer/src/main.ts session-viewer/src/styles.css session-viewer/vite.config.ts` | none | 0 | `1211 -> 1218` | `yes` | `?? session-viewer/src/main.ts` |
| `shell-85512-1780647808056354000-171` | `grep/search output` | `rg -n 'function renderTimeline|function renderTimelineItem|function buildFindings|type InvestigationFinding|type TimelineGroup' /Users/jiatwork/Works/context/so-context-main-session-sql/session-viewer/src/main.ts` | none | 0 | `1600 -> 1607` | `yes` | `79:type TimelineGroup = {` |
| `shell-85512-1780647808143215000-172` | `grep/search output` | `rg -n 'turn-group|timeline-item|finding-|badge-flow' /Users/jiatwork/Works/context/so-context-main-session-sql/session-viewer/src/styles.css` | none | 0 | `1601 -> 1608` | `yes` | `374:.timeline-item,` |
| `shell-85512-1780650752114842000-176` | `script output` | `python3 -c '<list rollout files script>'` | none | 0 | `1870 -> 1879` | `yes` | `/Users/jiatwork/.codex/sessions/2026/06/05/rollout-...` |
| `shell-85512-1780650752210514000-177` | `script output` | `python3 -c '<jsonl counter script>'` | none | 0 | `1872 -> 1881` | `yes` | `TOP Counter({'response_item': 1049, ...})` |

### Interpretation

These cases show two separate phenomena:

1. The initial `so_shell` result was already uncompressed text.
2. The agent still chose to call `so_shell_output` anyway.

That makes these cases useful for analyzing tool-contract and trust issues,
not compression-quality issues.

### Why These Cases Fell Back To Raw

The deciding rule lives in
`src/shell/runner.rs`: when `compressed_rendered.len() >= full_output.len()`,
`so_shell` returns `raw_fallback` instead of the compressed rendering.

For the `15` cases above, the compressed path usually did run, but it did not
produce a shorter string than the original output.

#### 1. `git status` on very small outputs

These runs go through the dedicated git status summarizer in
`src/shell/patterns/git/status.rs`.

For small outputs, the summarizer adds:

- a summary line such as
  `branch | staged=...; unstaged=...; untracked=...; changed_files=...`
- rewritten detail lines like `untracked: foo` or `unstaged: bar`

That is useful structure, but on tiny `git status --short` outputs it can be
longer than the raw porcelain lines, so the runner falls back to the original
text.

#### 2. `rg -n` searches with only a handful of hits

These runs go through the generic search summarizer in
`src/shell/patterns/generic/search.rs`.

For short hit sets, it keeps every hit, then adds:

- a summary line like `N matches in M files`
- a rewritten hit format `path:line — snippet`
- snippet cleanup/truncation logic

When there are only a few hits and the raw output is already compact, that
extra formatting can cost more bytes than it saves, especially with long
absolute paths.

#### 3. `python3` / `sqlite3` / `lsof` / shell-wrapper commands using generic fallback

These runs are not using a command-specific compressor. They fall through to
generic fallback in `src/shell/patterns/generic/mod.rs`.

Generic fallback works like this:

- keep all non-empty lines when line count is below a passthrough threshold
- add a summary line such as `N lines | KB`
- keep all shown lines unchanged

That means for many short outputs it is effectively "raw output plus one extra
summary line", which is guaranteed to be longer than the original output.

This affects:

- `sqlite3 ... .schema`
- `python3 -c ...`
- `lsof ...`
- `sh -c ...` wrappers such as `find ...`, `git rev-parse && git status`, and
  similar composite shell commands

#### 4. Shell wrappers are not classified by inner command

Commands like `sh -c 'find ...'` or `sh -c 'git ...'` are classified as `sh`,
not as `find` or `git`.

Because of that, they miss the more specialized `find` or `git` summarizers
and instead use generic fallback with a large passthrough threshold. That
makes `raw_fallback` much more likely on short or medium outputs.

### Practical Meaning

So the raw-fallback set is not one bug; it is a combination of:

1. some specialized summaries being too verbose for very small outputs
2. generic fallback preserving too many lines before deciding whether it was a
   useful compression
3. shell-wrapper commands not being classified by their inner command at all

## Part 2: `compressed` Cases

### Result

- `14` cases matched `compressed -> so_shell_output`.
- These are the cases where the initial `so_shell` result was actually
  summarized and the agent later asked for the fuller text payload.

### Category Summary

| Category | Count |
| --- | ---: |
| `code/config excerpt` | 8 |
| `grep/search output` | 3 |
| `test output` | 2 |
| `file listing` | 1 |

### Cases

| Run ID | Category | Command | CWD | Exit | JSONL lines | Summary vs raw | Likely missing detail |
| --- | --- | --- | --- | ---: | --- | --- | --- |
| `shell-85512-1780609254250855000-43` | `code/config excerpt` | `sh -c 'nl -ba /Users/jiatwork/.codex/config.toml \| sed -n 180,320p'` | `/Users/jiatwork/Works/context` | 0 | `65 -> 70` | `3846B -> 6276B` | exact config lines and tail content after the summarized excerpt |
| `shell-85512-1780610376138352000-55` | `code/config excerpt` | `sh -c 'nl -ba /Users/jiatwork/Works/context/so-context/src/hook/pre_tool.rs \| sed -n 1,120p'` | `/Users/jiatwork/Works/context` | 0 | `132 -> 139` | `2519B -> 4080B` | exact source lines for direct inspection and quoting |
| `shell-85512-1780610376190191000-56` | `code/config excerpt` | `sh -c 'nl -ba /Users/jiatwork/.codex/config.toml \| sed -n 250,320p'` | `/Users/jiatwork/Works/context` | 0 | `134 -> 140` | `2527B -> 2601B` | exact config strings and line-preserving output |
| `shell-85512-1780627239255566000-104` | `test output` | `cargo test pre_tool` | `/Users/jiatwork/Works/context/so-context` | 0 | `184 -> 194` | `92B -> 1397B` | individual test names and raw test runner output |
| `shell-85512-1780627260537664000-106` | `code/config excerpt` | `sh -c 'nl -ba /Users/jiatwork/Works/context/so-context/src/hook/pre_tool.rs \| sed -n 82,150p'` | `/Users/jiatwork/Works/context` | 0 | `201 -> 205` | `2620B -> 2700B` | exact source lines and formatting |
| `shell-85512-1780627427666197000-109` | `file listing` | `rg --files /Users/jiatwork/Works/context/so-context/src` | `/Users/jiatwork/Works/context` | 0 | `266 -> 279` | `4140B -> 6003B` | full file list without omitted tail entries |
| `shell-85512-1780630905421367000-115` | `grep/search output` | `rg -n 'session list\|list_sessions\|session history\|session_index\|history.jsonl\|events.db' /Users/jiatwork/Works/context --glob !target` | `/Users/jiatwork/Works/context` | 0 | `423 -> 432` | `8924B -> 9432B` | untruncated match lines without ellipsis |
| `shell-85512-1780630950731153000-118` | `grep/search output` | `rg -n 'session_id\|events.db\|so_shell_output\|mcp_tool_call_end\|history.jsonl\|session_index' /Users/jiatwork/Works/context/so-context/src /Users/jiatwork/Works/context/so-context/scripts --glob !target` | `/Users/jiatwork/Works/context` | 0 | `459 -> 484` | `7533B -> 20464B` | complete hit set and full match lines |
| `shell-85512-1780632402997641000-139` | `grep/search output` | `rg -n '_so_session_id\|agent_id\|session_id\|session_source\|agent =' /Users/jiatwork/Works/context/so-context-main-session-sql/src --glob !target` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 0 | `716 -> 731` | `8194B -> 20010B` | complete hit set and full match lines |
| `shell-85512-1780632435426220000-141` | `code/config excerpt` | `sed -n 200,420p /Users/jiatwork/Works/context/so-context-main-session-sql/src/mcp/mod.rs` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 0 | `765 -> 772` | `4253B -> 9426B` | exact Rust source lines across a long excerpt |
| `shell-85512-1780632435467021000-142` | `code/config excerpt` | `sed -n 1,220p /Users/jiatwork/Works/context/so-context-main-session-sql/src/mcp/tools/shell.rs` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 0 | `767 -> 773` | `4154B -> 7749B` | exact Rust source lines across a long excerpt |
| `shell-85512-1780632717139605000-146` | `test output` | `cargo test` | `/Users/jiatwork/Works/context/so-context-main-session-sql` | 101 | `843 -> 850` | `453B -> 9374B` | compiler/test failure details beyond aggregated error summary |
| `shell-85512-1780653251560088000-178` | `code/config excerpt` | `sed -n 760,1180p /Users/jiatwork/Works/context/so-context-main-session-sql/session-viewer/src/main.ts` | none | 0 | `1987 -> 1995` | `3122B -> 13771B` | exact TypeScript lines across a long excerpt |
| `shell-85512-1780653251608548000-179` | `code/config excerpt` | `sed -n 1180,1540p /Users/jiatwork/Works/context/so-context-main-session-sql/session-viewer/src/main.ts` | none | 0 | `1988 -> 1996` | `3915B -> 12541B` | exact TypeScript lines across a long excerpt |

### Interpretation

These cases are the real compression-quality set from this session.

The pattern is concentrated in four command families:

1. long code/config excerpts where the agent needs exact lines, not a shaped
   summary
2. `rg -n` searches where ellipsis or omitted hits make it hard to judge
   relevance
3. test runs where totals are not enough and raw test/compile output matters
4. file listings where the omitted tail prevents the next selection step

### Current Status After Fixes

The current implementation has materially changed how these commands render:

- text excerpts now use `CompressionSummary::plain(...)` and preserve exact
  output lines instead of reshaping them
- `rg --files` now has a dedicated plain-listing path with a `200`-file
  passthrough threshold
- `rg -n` / `grep` now keep original match lines for shown results instead of
  truncating snippets or moving the first hit into a synthetic summary line
- `rg -n` / `grep` now also use RTK-style caps: `200` total shown matches and
  `25` shown matches per file
- `cargo test` now includes successful test names and grouped compiler error
  blocks
- when compressed text is identical to full output, `so_shell` no longer
  advertises a `so_shell_output` follow-up

### Status By Case

| Run ID | Current status | Why |
| --- | --- | --- |
| `shell-85512-1780609254250855000-43` | `fixed` | `sh -c 'nl -ba ... \| sed -n ...'` is now classified as `TextExcerpt` and returned as verbatim plain text. |
| `shell-85512-1780610376138352000-55` | `fixed` | Same as `43`; exact excerpt lines are preserved and the current text matches full output. |
| `shell-85512-1780610376190191000-56` | `fixed` | Same as `43`; this case mainly needed line-preserving output, which is now the default excerpt path. |
| `shell-85512-1780627239255566000-104` | `fixed` | Successful `cargo test pre_tool` now keeps test names when the run is small enough, instead of collapsing to a totals-only line. |
| `shell-85512-1780627260537664000-106` | `fixed` | Same as `55`; exact source formatting is preserved by the excerpt path. |
| `shell-85512-1780627427666197000-109` | `fixed` | `rg --files` now uses a dedicated plain-listing path, and the current repo has `87` files under `src`, below the `200`-file passthrough threshold. |
| `shell-85512-1780630905421367000-115` | `fixed` | This query currently returns `77` hits, below the new `200`-match global cap, and its heaviest file is below the new `25`-match per-file cap, so the shown output now matches full output. |
| `shell-85512-1780630950731153000-118` | `fixed` | This query currently returns `152` hits, below the new `200`-match global cap, and its heaviest file is below the new `25`-match per-file cap, so the shown output now matches full output. |
| `shell-85512-1780632402997641000-139` | `fixed` | This query currently returns `171` hits, below the new `200`-match global cap, and its heaviest file is below the new `25`-match per-file cap, so the shown output now matches full output. |
| `shell-85512-1780632435426220000-141` | `fixed` | Direct `sed -n ...` excerpts now go through `TextExcerpt` and preserve exact Rust source lines. |
| `shell-85512-1780632435467021000-142` | `fixed` | Same as `141`; exact excerpt lines are now preserved. |
| `shell-85512-1780632717139605000-146` | `fixed` | Failing `cargo test` now uses RTK-style compile-error handling: summary includes error/warning counts plus compiled-crate count, and details preserve up to `15` full multiline compiler error blocks with an omitted-tail marker when needed. |
| `shell-85512-1780653251560088000-178` | `fixed` | Long `sed -n ...` TypeScript excerpts now preserve exact lines instead of a shaped summary. |
| `shell-85512-1780653251608548000-179` | `fixed` | Same as `178`; exact excerpt lines are now preserved. |

### Net Result

- `14 / 14` cases are now effectively fixed.

There are no remaining known `compressed -> so_shell_output` gaps from this
session after the current shell summarizer changes.

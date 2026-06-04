#!/usr/bin/env bash
set -euo pipefail

setup_gh_public_workspace() {
  local base="$1"
  local gh_bin
  gh_bin="$(command -v gh)"
  mkdir -p "$base/repo" "$base/bin" "$base/gh-config"
  cat >"$base/bin/gh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export GH_TOKEN=
export GH_ENTERPRISE_TOKEN=
export GH_PAGER=cat
exec "$gh_bin" "\$@"
EOF
  chmod +x "$base/bin/gh"
}

register_gh_cases() {
  if ! command_exists gh; then
    write_skip_case "$output_root/patterns/gh.pr/pr-list" "GitHub PR list" "gh.pr" "rtk-gh" "gh pr list --repo cli/cli --limit 3" "gh is not installed in this environment." "native gh binary not available"
    write_skip_case "$output_root/patterns/gh.issue/issue-list" "GitHub issue list" "gh.issue" "rtk-gh" "gh issue list --repo cli/cli --limit 3" "gh is not installed in this environment." "native gh binary not available"
    write_skip_case "$output_root/patterns/gh.run/run-list" "GitHub run list" "gh.run" "rtk-gh" "gh run list --repo cli/cli --limit 3" "gh is not installed in this environment." "native gh binary not available"
    return
  fi

  capture_rtk_dedicated_case "pr-list" "GitHub PR list" "gh.pr" \
    "Public PR list from cli/cli using anonymous gh access." \
    setup_gh_public_workspace "repo" "gh pr list --repo cli/cli --limit 3" "gh" \
    gh pr list --repo cli/cli --limit 3
  capture_rtk_dedicated_case "issue-list" "GitHub issue list" "gh.issue" \
    "Public issue list from cli/cli using anonymous gh access." \
    setup_gh_public_workspace "repo" "gh issue list --repo cli/cli --limit 3" "gh" \
    gh issue list --repo cli/cli --limit 3
  capture_rtk_dedicated_case "run-list" "GitHub run list" "gh.run" \
    "Public workflow run list from cli/cli using anonymous gh access." \
    setup_gh_public_workspace "repo" "gh run list --repo cli/cli --limit 3" "gh" \
    gh run list --repo cli/cli --limit 3
}

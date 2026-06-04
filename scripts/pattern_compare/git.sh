#!/usr/bin/env bash
set -euo pipefail

setup_status_dirty() {
  local base="$1"
  init_repo "$base/repo"
  printf 'staged\n' >>"$base/repo/README.md"
  printf 'unstaged\n' >>"$base/repo/src/app.txt"
  printf 'draft\n' >"$base/repo/new.txt"
  git -C "$base/repo" add README.md
}

setup_diff_unstaged() {
  local base="$1"
  init_repo "$base/repo"
  printf 'gamma\n' >>"$base/repo/src/app.txt"
}

setup_log_history() {
  local base="$1"
  init_repo "$base/repo"
  printf 'one\n' >>"$base/repo/README.md"
  commit_all "$base/repo" "docs: update readme"
  printf 'two\n' >>"$base/repo/src/app.txt"
  commit_all "$base/repo" "feat: expand app"
}

setup_branch_with_remote() {
  local base="$1"
  git init --bare "$base/remote.git" >/dev/null
  init_repo "$base/source"
  git -C "$base/source" remote add origin ../remote.git
  git -C "$base/source" push -u origin main >/dev/null
  git clone "$base/source/.git" "$base/repo" >/dev/null 2>&1
  git_user "$base/repo"
  git -C "$base/repo" remote set-url origin ../remote.git
  git -C "$base/repo" checkout -b feat/local >/dev/null 2>&1
  git -C "$base/repo" checkout main >/dev/null 2>&1
  git -C "$base/source" checkout -b feat/remote >/dev/null 2>&1
  printf 'remote branch\n' >>"$base/source/README.md"
  commit_all "$base/source" "feat: remote branch"
  git -C "$base/source" push -u origin feat/remote >/dev/null
  git -C "$base/repo" fetch origin >/dev/null 2>&1
}

setup_remote_verbose() {
  local base="$1"
  init_repo "$base/repo"
  git -C "$base/repo" remote add origin git@github.com:vestin-io/so-context.git
  git -C "$base/repo" remote add upstream git@github.com:example/upstream.git
}

setup_branch_transition() {
  local base="$1"
  init_repo "$base/repo"
  git -C "$base/repo" checkout -b feat/shell >/dev/null 2>&1
  git -C "$base/repo" checkout main >/dev/null 2>&1
}

setup_fetch_new_ref() {
  local base="$1"
  git init --bare "$base/remote.git" >/dev/null
  init_repo "$base/source"
  git -C "$base/source" remote add origin ../remote.git
  git -C "$base/source" push -u origin main >/dev/null
  git clone "$base/remote.git" "$base/repo" >/dev/null 2>&1
  git_user "$base/repo"
  git -C "$base/repo" remote set-url origin ../remote.git
  git -C "$base/source" checkout -b feat/fetch >/dev/null 2>&1
  printf 'fetch branch\n' >>"$base/source/README.md"
  commit_all "$base/source" "feat: fetch branch"
  git -C "$base/source" push -u origin feat/fetch >/dev/null
  git -C "$base/source" tag v1.1.0
  git -C "$base/source" push origin v1.1.0 >/dev/null
}

setup_pull_fast_forward() {
  local base="$1"
  git init --bare "$base/remote.git" >/dev/null
  init_repo "$base/source"
  git -C "$base/source" remote add origin ../remote.git
  git -C "$base/source" push -u origin main >/dev/null
  git clone "$base/remote.git" "$base/repo" >/dev/null 2>&1
  git_user "$base/repo"
  git -C "$base/repo" remote set-url origin ../remote.git
  printf 'upstream change\n' >>"$base/source/src/app.txt"
  commit_all "$base/source" "feat: upstream change"
  git -C "$base/source" push origin main >/dev/null
}

setup_push_ahead() {
  local base="$1"
  git init --bare "$base/remote.git" >/dev/null
  init_repo "$base/source"
  git -C "$base/source" remote add origin ../remote.git
  git -C "$base/source" push -u origin main >/dev/null
  git clone "$base/remote.git" "$base/repo" >/dev/null 2>&1
  git_user "$base/repo"
  git -C "$base/repo" remote set-url origin ../remote.git
  printf 'local ahead\n' >>"$base/repo/src/app.txt"
  commit_all "$base/repo" "feat: local ahead"
}

setup_add_paths() {
  local base="$1"
  init_repo "$base/repo"
  printf 'delta\n' >>"$base/repo/README.md"
  printf 'draft\n' >"$base/repo/notes.txt"
}

setup_commit_staged() {
  local base="$1"
  init_repo "$base/repo"
  printf 'release\n' >>"$base/repo/README.md"
  git -C "$base/repo" add README.md
}

setup_stash_save() {
  local base="$1"
  init_repo "$base/repo"
  printf 'scratch\n' >>"$base/repo/src/app.txt"
  printf 'notes\n' >"$base/repo/draft.txt"
}

setup_stash_list() {
  local base="$1"
  init_repo "$base/repo"
  printf 'scratch\n' >>"$base/repo/src/app.txt"
  git -C "$base/repo" stash push -m "demo stash" >/dev/null
}

setup_clone_basic() {
  local base="$1"
  git init --bare "$base/remote.git" >/dev/null
  init_repo "$base/source"
  git -C "$base/source" remote add origin ../remote.git
  git -C "$base/source" push -u origin main >/dev/null
  mkdir -p "$base/work"
}

setup_merge_conflict() {
  local base="$1"
  init_repo "$base/repo"
  cat >"$base/repo/src/app.txt" <<'EOF'
main
shared
EOF
  commit_all "$base/repo" "base: shared line"
  git -C "$base/repo" checkout -b feat/conflict >/dev/null 2>&1
  cat >"$base/repo/src/app.txt" <<'EOF'
feature
shared
EOF
  commit_all "$base/repo" "feat: conflicting change"
  git -C "$base/repo" checkout main >/dev/null 2>&1
  cat >"$base/repo/src/app.txt" <<'EOF'
main
shared-updated
EOF
  commit_all "$base/repo" "fix: main change"
}

setup_tag_list() {
  local base="$1"
  init_repo "$base/repo"
  git -C "$base/repo" tag v1.0.0
  git -C "$base/repo" tag v1.1.0
  git -C "$base/repo" tag v2.0.0
}

setup_reset_unstage() {
  local base="$1"
  init_repo "$base/repo"
  printf 'reset one\n' >>"$base/repo/README.md"
  printf 'reset two\n' >>"$base/repo/src/app.txt"
  git -C "$base/repo" add README.md src/app.txt
}

register_git_cases() {
  capture_rtk_git_case "status-dirty" "Dirty working tree" "git.status" \
    "Staged, unstaged, and untracked changes in the same repo state." \
    setup_status_dirty "repo" "git status" git status
  capture_rtk_git_case "diff-unstaged" "Unstaged diff" "git.diff" \
    "Single tracked file changed without staging." \
    setup_diff_unstaged "repo" "git diff" git diff
  capture_rtk_git_case "log-history" "Commit history" "git.log" \
    "Three commits of history with plain git log output." \
    setup_log_history "repo" "git log -n 3" git log -n 3
  capture_rtk_git_case "show-head" "Show HEAD" "git.show" \
    "Full git show for the latest commit." \
    setup_log_history "repo" "git show HEAD" git show HEAD
  capture_rtk_git_case "branch-with-remote" "Branch listing" "git.branch" \
    "Local branch plus fetched remote-tracking branch." \
    setup_branch_with_remote "repo" "git branch -a" git branch -a
  capture_rtk_git_case "remote-verbose" "Remote list" "git.remote" \
    "Two remotes listed with fetch/push URLs." \
    setup_remote_verbose "repo" "git remote -v" git remote -v
  capture_rtk_git_case "checkout-branch" "Checkout existing branch" "git.checkout" \
    "Switch from main to an existing local branch." \
    setup_branch_transition "repo" "git checkout feat/shell" git checkout feat/shell
  capture_rtk_git_case "switch-branch" "Switch existing branch" "git.switch" \
    "Switch from main to an existing local branch." \
    setup_branch_transition "repo" "git switch feat/shell" git switch feat/shell
  capture_rtk_git_case "fetch-new-ref" "Fetch new refs" "git.fetch" \
    "Remote has a new branch and tag before fetch." \
    setup_fetch_new_ref "repo" "git fetch --tags" git fetch --tags
  capture_rtk_git_case "pull-fast-forward" "Pull fast-forward" "git.pull" \
    "Local repo is behind remote main by one commit." \
    setup_pull_fast_forward "repo" "git pull" git pull
  capture_rtk_git_case "push-ahead" "Push ahead commit" "git.push" \
    "Local repo is ahead of remote main by one commit." \
    setup_push_ahead "repo" "git push origin main" git push origin main
  capture_rtk_git_case "add-paths" "Add explicit paths" "git.add" \
    "Two file paths are passed to git add." \
    setup_add_paths "repo" "git add README.md notes.txt" git add README.md notes.txt
  capture_rtk_git_case "commit-staged" "Commit staged change" "git.commit" \
    "One staged README change committed with a message." \
    setup_commit_staged "repo" "git commit -m demo commit" git commit -m "demo commit"
  capture_rtk_git_case "stash-save" "Stash save" "git.stash" \
    "Tracked and untracked working tree changes saved to stash with include-untracked." \
    setup_stash_save "repo" "git stash -u" git stash -u
  capture_rtk_git_case "stash-list" "Stash list" "git.stash" \
    "A repo with one existing stash entry." \
    setup_stash_list "repo" "git stash list" git stash list

  capture_rtk_summary_case "clone-basic" "Clone local bare remote" "git.clone" \
    "RTK uses the generic summary wrapper here because rtk git has no dedicated clone subcommand." \
    setup_clone_basic "work" "git clone ../remote.git cloned-repo" \
    git clone ../remote.git cloned-repo
  capture_rtk_summary_case "merge-conflict" "Merge conflict" "git.merge" \
    "RTK uses the generic summary wrapper here because rtk git has no dedicated merge subcommand." \
    setup_merge_conflict "repo" "git merge feat/conflict" \
    git merge feat/conflict
  capture_rtk_summary_case "tag-list" "Tag list" "git.tag" \
    "RTK uses the generic summary wrapper here because rtk git has no dedicated tag subcommand." \
    setup_tag_list "repo" "git tag" \
    git tag
  capture_rtk_summary_case "reset-unstage" "Reset staged files" "git.reset" \
    "RTK uses the generic summary wrapper here because rtk git has no dedicated reset subcommand." \
    setup_reset_unstage "repo" "git reset HEAD README.md src/app.txt" \
    git reset HEAD README.md src/app.txt
}

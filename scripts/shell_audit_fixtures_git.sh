#!/usr/bin/env bash

setup_git_fixture() {
  local dir="$fixture_root/git-fixture"
  local remote="$fixture_root/git-remote.git"
  rm -rf "$dir" "$remote"
  git init --bare "$remote" >/dev/null
  git init "$dir" >/dev/null
  (
    cd "$dir"
    git config user.name "Audit Bot"
    git config user.email "audit@example.com"
    printf 'first\n' >README.md
    git add README.md
    git commit -m "first commit" >/dev/null
    printf 'second\n' >>README.md
    git commit -am "second commit" >/dev/null
    git remote add origin "$remote"
    git branch -M main
    git push -u origin main >/dev/null 2>&1
    printf 'dirty\n' >>README.md
  )
  set_setup_paths "$dir" "$dir"
}

git_user() {
  git -C "$1" config user.name "so-context"
  git -C "$1" config user.email "dev@example.com"
}

init_git_repo() {
  local repo_dir="$1"
  mkdir -p "$repo_dir/src"
  git init -b main "$repo_dir" >/dev/null
  git_user "$repo_dir"
  cat >"$repo_dir/README.md" <<'EOF_INNER'
# Demo Repo
EOF_INNER
  cat >"$repo_dir/src/app.txt" <<'EOF_INNER'
alpha
beta
EOF_INNER
  git -C "$repo_dir" add README.md src/app.txt
  git -C "$repo_dir" commit -m "base" >/dev/null
}

git_commit_all() {
  local repo_dir="$1"
  local message="$2"
  git -C "$repo_dir" add .
  git -C "$repo_dir" commit -m "$message" >/dev/null
}

setup_git_status_dirty() {
  local dir="$fixture_root/git-status-dirty"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'staged\n' >>"$dir/repo/README.md"
  printf 'unstaged\n' >>"$dir/repo/src/app.txt"
  printf 'draft\n' >"$dir/repo/new.txt"
  git -C "$dir/repo" add README.md
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_diff_unstaged() {
  local dir="$fixture_root/git-diff-unstaged"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'gamma\n' >>"$dir/repo/src/app.txt"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_log_history() {
  local dir="$fixture_root/git-log-history"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'one\n' >>"$dir/repo/README.md"
  git_commit_all "$dir/repo" "docs: update readme"
  printf 'two\n' >>"$dir/repo/src/app.txt"
  git_commit_all "$dir/repo" "feat: expand app"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_branch_with_remote() {
  local dir="$fixture_root/git-branch-remote"
  mkdir -p "$dir"
  git init --bare "$dir/remote.git" >/dev/null
  init_git_repo "$dir/source"
  git -C "$dir/source" remote add origin ../remote.git
  git -C "$dir/source" push -u origin main >/dev/null
  git clone "$dir/source/.git" "$dir/repo" >/dev/null 2>&1
  git_user "$dir/repo"
  git -C "$dir/repo" remote set-url origin ../remote.git
  git -C "$dir/repo" checkout -b feat/local >/dev/null 2>&1
  git -C "$dir/repo" checkout main >/dev/null 2>&1
  git -C "$dir/source" checkout -b feat/remote >/dev/null 2>&1
  printf 'remote branch\n' >>"$dir/source/README.md"
  git_commit_all "$dir/source" "feat: remote branch"
  git -C "$dir/source" push -u origin feat/remote >/dev/null
  git -C "$dir/repo" fetch origin >/dev/null 2>&1
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_remote_verbose() {
  local dir="$fixture_root/git-remote-verbose"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  git -C "$dir/repo" remote add origin git@github.com:vestin-io/so-context.git
  git -C "$dir/repo" remote add upstream git@github.com:example/upstream.git
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_branch_transition() {
  local dir="$fixture_root/git-branch-transition"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  git -C "$dir/repo" checkout -b feat/shell >/dev/null 2>&1
  git -C "$dir/repo" checkout main >/dev/null 2>&1
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_fetch_new_ref() {
  local dir="$fixture_root/git-fetch"
  mkdir -p "$dir"
  git init --bare "$dir/remote.git" >/dev/null
  init_git_repo "$dir/source"
  git -C "$dir/source" remote add origin ../remote.git
  git -C "$dir/source" push -u origin main >/dev/null
  git clone "$dir/remote.git" "$dir/repo" >/dev/null 2>&1
  git_user "$dir/repo"
  git -C "$dir/repo" remote set-url origin ../remote.git
  git -C "$dir/source" checkout -b feat/fetch >/dev/null 2>&1
  printf 'fetch branch\n' >>"$dir/source/README.md"
  git_commit_all "$dir/source" "feat: fetch branch"
  git -C "$dir/source" push -u origin feat/fetch >/dev/null
  git -C "$dir/source" tag v1.1.0
  git -C "$dir/source" push origin v1.1.0 >/dev/null
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_pull_fast_forward() {
  local dir="$fixture_root/git-pull"
  mkdir -p "$dir"
  git init --bare "$dir/remote.git" >/dev/null
  init_git_repo "$dir/source"
  git -C "$dir/source" remote add origin ../remote.git
  git -C "$dir/source" push -u origin main >/dev/null
  git clone "$dir/remote.git" "$dir/repo" >/dev/null 2>&1
  git_user "$dir/repo"
  git -C "$dir/repo" remote set-url origin ../remote.git
  printf 'upstream change\n' >>"$dir/source/src/app.txt"
  git_commit_all "$dir/source" "feat: upstream change"
  git -C "$dir/source" push origin main >/dev/null
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_push_ahead() {
  local dir="$fixture_root/git-push"
  mkdir -p "$dir"
  git init --bare "$dir/remote.git" >/dev/null
  init_git_repo "$dir/source"
  git -C "$dir/source" remote add origin ../remote.git
  git -C "$dir/source" push -u origin main >/dev/null
  git clone "$dir/remote.git" "$dir/repo" >/dev/null 2>&1
  git_user "$dir/repo"
  git -C "$dir/repo" remote set-url origin ../remote.git
  printf 'local ahead\n' >>"$dir/repo/src/app.txt"
  git_commit_all "$dir/repo" "feat: local ahead"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_add_paths() {
  local dir="$fixture_root/git-add"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'delta\n' >>"$dir/repo/README.md"
  printf 'draft\n' >"$dir/repo/notes.txt"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_commit_staged() {
  local dir="$fixture_root/git-commit"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'release\n' >>"$dir/repo/README.md"
  git -C "$dir/repo" add README.md
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_stash_save() {
  local dir="$fixture_root/git-stash-save"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'scratch\n' >>"$dir/repo/src/app.txt"
  printf 'notes\n' >"$dir/repo/draft.txt"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_stash_list() {
  local dir="$fixture_root/git-stash-list"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'scratch\n' >>"$dir/repo/src/app.txt"
  git -C "$dir/repo" stash push -m "demo stash" >/dev/null
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_clone_basic() {
  local dir="$fixture_root/git-clone"
  mkdir -p "$dir"
  git init --bare "$dir/remote.git" >/dev/null
  init_git_repo "$dir/source"
  git -C "$dir/source" remote add origin ../remote.git
  git -C "$dir/source" push -u origin main >/dev/null
  mkdir -p "$dir/work"
  set_setup_paths "$dir" "$dir/work"
}

setup_git_merge_conflict() {
  local dir="$fixture_root/git-merge"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  cat >"$dir/repo/src/app.txt" <<'EOF_INNER'
main
shared
EOF_INNER
  git_commit_all "$dir/repo" "base: shared line"
  git -C "$dir/repo" checkout -b feat/conflict >/dev/null 2>&1
  cat >"$dir/repo/src/app.txt" <<'EOF_INNER'
feature
shared
EOF_INNER
  git_commit_all "$dir/repo" "feat: conflicting change"
  git -C "$dir/repo" checkout main >/dev/null 2>&1
  cat >"$dir/repo/src/app.txt" <<'EOF_INNER'
main
shared-updated
EOF_INNER
  git_commit_all "$dir/repo" "fix: main change"
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_tag_list() {
  local dir="$fixture_root/git-tag"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  git -C "$dir/repo" tag v1.0.0
  git -C "$dir/repo" tag v1.1.0
  git -C "$dir/repo" tag v2.0.0
  set_setup_paths "$dir" "$dir/repo"
}

setup_git_reset_unstage() {
  local dir="$fixture_root/git-reset"
  mkdir -p "$dir"
  init_git_repo "$dir/repo"
  printf 'reset one\n' >>"$dir/repo/README.md"
  printf 'reset two\n' >>"$dir/repo/src/app.txt"
  git -C "$dir/repo" add README.md src/app.txt
  set_setup_paths "$dir" "$dir/repo"
}

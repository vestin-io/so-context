#!/usr/bin/env bash
set -euo pipefail

ensure_binary() {
  (cd "$repo_root" && cargo build --quiet)
}

reset_output_dir() {
  rm -rf "$output_root"
  mkdir -p "$output_root/patterns"
}

write_readme() {
  cat >"$output_root/README.md" <<'EOF'
# Pattern Output Compare

This directory contains optional RTK reference compare output.

This is not the formal shell audit path.
Use `bash scripts/shell_audit.sh` for the canonical audit.

Layout:

- `patterns/<pattern>/<scenario>/`: compare cases grouped by `so-context` shell pattern label

Each scenario directory contains:

- `command.txt`: the exact command under test
- `notes.txt`: scenario description plus `pattern` and `rtk_mode`
- `runner-commands.txt`: exact native / RTK / so-context invocations used
- `native.txt`: raw command output
- `rtk.txt`: RTK output using either a dedicated `rtk <wrapper> ...` command or `rtk summary ...`
- `so-context.txt`: `so-context shell -- ...` output
- `exit-codes.txt`: exit code for each runner

Notes:

- Git scenarios run against live temporary repositories.
- Local-tool scenarios prefer real commands running against deterministic fixture workspaces.
- Some scenarios are intentionally marked `skip` when a fair RTK-specialized compare is not yet available in this environment.
EOF
}

join_cmd() {
  local out=""
  local arg
  for arg in "$@"; do
    if [ -n "$out" ]; then
      out+=" "
    fi
    out+="$arg"
  done
  printf '%s' "$out"
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

write_metadata() {
  local out_dir="$1"
  local title="$2"
  local pattern="$3"
  local rtk_mode="$4"
  local command="$5"
  local notes="$6"
  cat >"$out_dir/notes.txt" <<EOF
title: $title
pattern: $pattern
rtk_mode: $rtk_mode
command: $command

$notes
EOF
  printf '%s\n' "$command" >"$out_dir/command.txt"
}

write_runner_commands() {
  local out_dir="$1"
  local native_cmd="$2"
  local rtk_cmd="$3"
  local so_cmd="$4"
  cat >"$out_dir/runner-commands.txt" <<EOF
native: $native_cmd
rtk: $rtk_cmd
so-context: $so_cmd
EOF
}

write_exit_summary() {
  local out_dir="$1"
  {
    printf 'native: %s\n' "$(cat "$out_dir/native.exit")"
    printf 'rtk: %s\n' "$(cat "$out_dir/rtk.exit")"
    printf 'so-context: %s\n' "$(cat "$out_dir/so-context.exit")"
  } >"$out_dir/exit-codes.txt"
}

write_skip_case() {
  local out_dir="$1"
  local title="$2"
  local pattern="$3"
  local rtk_mode="$4"
  local command="$5"
  local notes="$6"
  local reason="$7"
  mkdir -p "$out_dir"
  write_metadata "$out_dir" "$title" "$pattern" "$rtk_mode" "$command" "$notes"
  printf '%s\n' "skipped: $reason" >"$out_dir/native.txt"
  printf '%s\n' "skipped: $reason" >"$out_dir/rtk.txt"
  printf '%s\n' "skipped: $reason" >"$out_dir/so-context.txt"
  printf '%s\n' "native: skip" >"$out_dir/exit-codes.txt"
  printf '%s\n' "rtk: skip" >>"$out_dir/exit-codes.txt"
  printf '%s\n' "so-context: skip" >>"$out_dir/exit-codes.txt"
  printf '%s\n' "skip" >"$out_dir/native.exit"
  printf '%s\n' "skip" >"$out_dir/rtk.exit"
  printf '%s\n' "skip" >"$out_dir/so-context.exit"
  printf '%s\n' "skip_reason: $reason" >"$out_dir/runner-commands.txt"
}

runner_path_value() {
  local runner="$1"
  if [ -d "$runner/bin" ]; then
    printf '%s/bin:%s' "$runner" "$PATH"
  else
    printf '%s' "$PATH"
  fi
}

run_capture() {
  local cwd="$1"
  local out_file="$2"
  local code_file="$3"
  local path_value="$4"
  shift 4
  set +e
  (
    cd "$cwd"
    PATH="$path_value" "$@"
  ) >"$out_file" 2>&1
  local code=$?
  set -e
  printf '%s\n' "$code" >"$code_file"
}

capture_native() {
  local base_dir="$1"
  local cwd_rel="$2"
  local out_dir="$3"
  shift 3
  local runner="$work_root/runner-native-$(basename "$out_dir")"
  cp -R "$base_dir" "$runner"
  run_capture \
    "$runner/$cwd_rel" \
    "$out_dir/native.txt" \
    "$out_dir/native.exit" \
    "$(runner_path_value "$runner")" \
    "$@"
  rm -rf "$runner"
}

capture_rtk_git() {
  local base_dir="$1"
  local cwd_rel="$2"
  local out_dir="$3"
  shift 3
  local runner="$work_root/runner-rtk-$(basename "$out_dir")"
  cp -R "$base_dir" "$runner"
  run_capture \
    "$runner/$cwd_rel" \
    "$out_dir/rtk.txt" \
    "$out_dir/rtk.exit" \
    "$(runner_path_value "$runner")" \
    rtk git "$@"
  rm -rf "$runner"
}

capture_rtk_dedicated() {
  local wrapper="$1"
  local base_dir="$2"
  local cwd_rel="$3"
  local out_dir="$4"
  shift 4
  local runner="$work_root/runner-rtk-$(basename "$out_dir")"
  cp -R "$base_dir" "$runner"
  run_capture \
    "$runner/$cwd_rel" \
    "$out_dir/rtk.txt" \
    "$out_dir/rtk.exit" \
    "$(runner_path_value "$runner")" \
    rtk "$wrapper" "$@"
  rm -rf "$runner"
}

capture_rtk_summary() {
  local base_dir="$1"
  local cwd_rel="$2"
  local out_dir="$3"
  shift 3
  local runner="$work_root/runner-rtk-$(basename "$out_dir")"
  cp -R "$base_dir" "$runner"
  run_capture \
    "$runner/$cwd_rel" \
    "$out_dir/rtk.txt" \
    "$out_dir/rtk.exit" \
    "$(runner_path_value "$runner")" \
    rtk summary "$@"
  rm -rf "$runner"
}

capture_so() {
  local base_dir="$1"
  local cwd_rel="$2"
  local out_dir="$3"
  shift 3
  local runner="$work_root/runner-so-$(basename "$out_dir")"
  cp -R "$base_dir" "$runner"
  run_capture \
    "$runner/$cwd_rel" \
    "$out_dir/so-context.txt" \
    "$out_dir/so-context.exit" \
    "$(runner_path_value "$runner")" \
    "$binary" shell -- "$@"
  rm -rf "$runner"
}

capture_rtk_git_case() {
  local name="$1"
  local title="$2"
  local pattern="$3"
  local notes="$4"
  local setup_fn="$5"
  local cwd_rel="$6"
  local command_display="$7"
  shift 7
  local command=("$@")
  local base="$work_root/base-$name"
  local out_dir="$output_root/patterns/$pattern/$name"
  mkdir -p "$base" "$out_dir"
  "$setup_fn" "$base"
  write_metadata "$out_dir" "$title" "$pattern" "rtk-git" "$command_display" "$notes"
  write_runner_commands \
    "$out_dir" \
    "$(join_cmd "${command[@]}")" \
    "rtk git $(join_cmd "${command[@]:1}")" \
    "so-context shell -- $(join_cmd "${command[@]}")"
  capture_native "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  capture_rtk_git "$base" "$cwd_rel" "$out_dir" "${command[@]:1}"
  capture_so "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  write_exit_summary "$out_dir"
}

capture_rtk_summary_case() {
  local name="$1"
  local title="$2"
  local pattern="$3"
  local notes="$4"
  local setup_fn="$5"
  local cwd_rel="$6"
  local command_display="$7"
  shift 7
  local command=("$@")
  local base="$work_root/base-$name"
  local out_dir="$output_root/patterns/$pattern/$name"
  mkdir -p "$base" "$out_dir"
  "$setup_fn" "$base"
  write_metadata "$out_dir" "$title" "$pattern" "rtk-summary" "$command_display" "$notes"
  write_runner_commands \
    "$out_dir" \
    "$(join_cmd "${command[@]}")" \
    "rtk summary $(join_cmd "${command[@]}")" \
    "so-context shell -- $(join_cmd "${command[@]}")"
  capture_native "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  capture_rtk_summary "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  capture_so "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  write_exit_summary "$out_dir"
}

capture_rtk_dedicated_case() {
  local name="$1"
  local title="$2"
  local pattern="$3"
  local notes="$4"
  local setup_fn="$5"
  local cwd_rel="$6"
  local command_display="$7"
  local wrapper="$8"
  shift 8
  local command=("$@")
  local base="$work_root/base-$name"
  local out_dir="$output_root/patterns/$pattern/$name"
  mkdir -p "$base" "$out_dir"
  "$setup_fn" "$base"
  write_metadata "$out_dir" "$title" "$pattern" "rtk-$wrapper" "$command_display" "$notes"
  write_runner_commands \
    "$out_dir" \
    "$(join_cmd "${command[@]}")" \
    "rtk $wrapper $(join_cmd "${command[@]:1}")" \
    "so-context shell -- $(join_cmd "${command[@]}")"
  capture_native "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  capture_rtk_dedicated "$wrapper" "$base" "$cwd_rel" "$out_dir" "${command[@]:1}"
  capture_so "$base" "$cwd_rel" "$out_dir" "${command[@]}"
  write_exit_summary "$out_dir"
}

git_user() {
  git -C "$1" config user.name "so-context"
  git -C "$1" config user.email "dev@example.com"
}

init_repo() {
  local repo_dir="$1"
  mkdir -p "$repo_dir/src"
  git init -b main "$repo_dir" >/dev/null
  git_user "$repo_dir"
  cat >"$repo_dir/README.md" <<'EOF'
# Demo Repo
EOF
  cat >"$repo_dir/src/app.txt" <<'EOF'
alpha
beta
EOF
  git -C "$repo_dir" add README.md src/app.txt
  git -C "$repo_dir" commit -m "base" >/dev/null
}

commit_all() {
  local repo_dir="$1"
  local message="$2"
  git -C "$repo_dir" add .
  git -C "$repo_dir" commit -m "$message" >/dev/null
}

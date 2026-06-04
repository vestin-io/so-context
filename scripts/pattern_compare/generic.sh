#!/usr/bin/env bash
set -euo pipefail

setup_generic_workspace() {
  local base="$1"
  mkdir -p "$base/repo/src/shell"
  cat >"$base/repo/Cargo.toml" <<'EOF'
[package]
name = "fixture"
version = "0.1.0"
EOF
  cat >"$base/repo/README.md" <<'EOF'
# so-context
intro line
second line
EOF
  cat >"$base/repo/src/main.rs" <<'EOF'
fn main() {
    // TODO one
}
EOF
  cat >"$base/repo/src/lib.rs" <<'EOF'
// TODO two
EOF
  cat >"$base/repo/src/shell/mod.rs" <<'EOF'
pub mod mod_file;
EOF
}

register_generic_cases() {
  capture_rtk_dedicated_case "ls-basic" "List directory entries" "generic.ls" \
    "Three top-level entries from ls." \
    setup_generic_workspace "repo" "ls -1" "ls" \
    ls -1
  capture_rtk_dedicated_case "find-src" "Find source files" "generic.find" \
    "Source tree paths under src." \
    setup_generic_workspace "repo" "find src -type f" "find" \
    find src -type f
  capture_rtk_summary_case "rg-todos" "Ripgrep TODOs" "generic.rg" \
    "Two TODO matches across two files. RTK has no dedicated rg wrapper." \
    setup_generic_workspace "repo" "rg TODO" \
    rg TODO
  capture_rtk_dedicated_case "grep-todos" "Grep TODOs" "generic.grep" \
    "Recursive grep output with two TODO matches." \
    setup_generic_workspace "repo" "grep -R TODO ." "grep" \
    grep -R TODO .
  capture_rtk_dedicated_case "env-vars" "Environment snapshot" "generic.env" \
    "PATH plus one redacted secret and one normal variable." \
    setup_generic_workspace "repo" "env -i PATH=/usr/bin:/bin AWS_SECRET_ACCESS_KEY=super-secret RUST_LOG=debug env" "env" \
    env -i PATH=/usr/bin:/bin AWS_SECRET_ACCESS_KEY=super-secret RUST_LOG=debug env
  capture_rtk_summary_case "cat-readme" "Cat README" "generic.cat" \
    "Three lines from a README file. RTK has no dedicated cat wrapper." \
    setup_generic_workspace "repo" "cat README.md" \
    cat README.md
  capture_rtk_summary_case "head-readme" "Head README" "generic.head" \
    "First two lines from a README file. RTK has no dedicated head wrapper." \
    setup_generic_workspace "repo" "head -n 2 README.md" \
    head -n 2 README.md
  capture_rtk_summary_case "tail-readme" "Tail README" "generic.tail" \
    "Last two lines from a file. RTK has no dedicated tail wrapper." \
    setup_generic_workspace "repo" "tail -n 2 README.md" \
    tail -n 2 README.md
  capture_rtk_dedicated_case "curl-body" "Curl text response" "generic.curl" \
    "Two-line text body from a local file URL fixture." \
    setup_generic_workspace "repo" "curl -s file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-text.txt" "curl" \
    curl -s "file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-text.txt"
  capture_rtk_dedicated_case "curl-json" "Curl JSON response" "generic.curl" \
    "Single-line JSON body from a local file URL fixture." \
    setup_generic_workspace "repo" "curl -s file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-json.txt" "curl" \
    curl -s "file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-json.txt"

  if command_exists wget; then
    capture_rtk_dedicated_case "wget-download" "Wget download" "generic.wget" \
      "Download summary from a data URL." \
      setup_generic_workspace "repo" "wget data:text/plain,hello" "wget" \
      wget "data:text/plain,hello"
  else
    write_skip_case \
      "$output_root/patterns/generic.wget/wget-download" \
      "Wget download" \
      "generic.wget" \
      "rtk-wget" \
      "wget data:text/plain,hello" \
      "wget is not installed in this environment." \
      "native wget binary not available"
  fi
}

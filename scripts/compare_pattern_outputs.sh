#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_root="${1:-$repo_root/.so-context/reference-compare}"
binary="$repo_root/target/debug/so-context"
work_root="$(mktemp -d "${TMPDIR:-/tmp}/so-context-pattern-compare.XXXXXX")"

cleanup() {
  if declare -f docker_cleanup_live_fixture >/dev/null 2>&1; then
    docker_cleanup_live_fixture || true
  fi
  if declare -f k8s_cleanup_live_fixture >/dev/null 2>&1; then
    k8s_cleanup_live_fixture || true
  fi
  rm -rf "$work_root"
}
trap cleanup EXIT

source "$repo_root/scripts/pattern_compare/common.sh"
source "$repo_root/scripts/pattern_compare/git.sh"
source "$repo_root/scripts/pattern_compare/docker.sh"
source "$repo_root/scripts/pattern_compare/generic.sh"
source "$repo_root/scripts/pattern_compare/node.sh"
source "$repo_root/scripts/pattern_compare/rust.sh"
source "$repo_root/scripts/pattern_compare/gh.sh"
source "$repo_root/scripts/pattern_compare/k8s.sh"
source "$repo_root/scripts/pattern_compare/build.sh"

main() {
  ensure_binary
  reset_output_dir
  write_readme

  register_git_cases
  register_docker_cases
  register_generic_cases
  register_node_cases
  register_rust_cases
  register_gh_cases
  register_k8s_cases
  register_build_cases
}

main "$@"

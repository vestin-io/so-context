#!/usr/bin/env bash
set -euo pipefail

setup_rust_workspace() {
  local base="$1"
  mkdir -p "$base/repo/src" "$base/repo/tests"
  cat >"$base/repo/Cargo.toml" <<'EOF'
[package]
name = "pattern_compare_fixture"
version = "0.1.0"
edition = "2021"
EOF
  cat >"$base/repo/src/lib.rs" <<'EOF'
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::add;

    #[test]
    fn adds_numbers() {
        assert_eq!(add(1, 1), 2);
    }
}
EOF
  cat >"$base/repo/src/main.rs" <<'EOF'
fn main() {
    println!("{}", pattern_compare_fixture::add(20, 22));
}
EOF
  cat >"$base/repo/tests/smoke.rs" <<'EOF'
use pattern_compare_fixture::add;

#[test]
fn smoke() {
    assert_eq!(add(2, 2), 4);
}
EOF
}

register_rust_cases() {
  capture_rtk_dedicated_case "cargo-build" "cargo build" "rust.cargo-build" \
    "Successful cargo build on a local fixture crate." \
    setup_rust_workspace "repo" "cargo build" "cargo" \
    cargo build
  capture_rtk_dedicated_case "cargo-test" "cargo test" "rust.cargo-test" \
    "Passing cargo test run on a local fixture crate." \
    setup_rust_workspace "repo" "cargo test" "cargo" \
    cargo test
  capture_rtk_dedicated_case "cargo-check" "cargo check" "rust.cargo-check" \
    "Successful cargo check on a local fixture crate." \
    setup_rust_workspace "repo" "cargo check" "cargo" \
    cargo check
  capture_rtk_dedicated_case "cargo-clippy" "cargo clippy" "rust.cargo-clippy" \
    "cargo clippy run on a local fixture crate." \
    setup_rust_workspace "repo" "cargo clippy --all-targets --no-deps" "cargo" \
    cargo clippy --all-targets --no-deps
  capture_rtk_dedicated_case "cargo-install" "cargo install --path ." "rust.cargo-install" \
    "Local path install into a temp root." \
    setup_rust_workspace "repo" "cargo install --path . --root ./install-root --force" "cargo" \
    cargo install --path . --root ./install-root --force

  if cargo nextest --version >/dev/null 2>&1; then
    capture_rtk_dedicated_case "cargo-nextest" "cargo nextest run" "rust.cargo-nextest" \
      "Successful cargo nextest run on a local fixture crate." \
      setup_rust_workspace "repo" "cargo nextest run" "cargo" \
      cargo nextest run
  else
    write_skip_case \
      "$output_root/patterns/rust.cargo-nextest/cargo-nextest" \
      "cargo nextest run" \
      "rust.cargo-nextest" \
      "rtk-cargo" \
      "cargo nextest run" \
      "cargo-nextest is not installed in this environment." \
      "cargo nextest subcommand not available"
  fi
}

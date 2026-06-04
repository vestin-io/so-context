#!/usr/bin/env bash

SETUP_ROOT=""
SETUP_CWD=""

clear_setup_paths() {
  SETUP_ROOT=""
  SETUP_CWD=""
}

set_setup_paths() {
  SETUP_ROOT="$1"
  SETUP_CWD="$2"
}

setup_ls_fixture() {
  local dir="$fixture_root/ls-fixture"
  mkdir -p "$dir/src"
  printf 'hello' >"$dir/alpha.txt"
  printf 'world' >"$dir/beta.txt"
  set_setup_paths "$dir" "$dir"
}

setup_generic_fixture() {
  local dir="$fixture_root/generic-fixture"
  mkdir -p "$dir/src/shell"
  cat >"$dir/Cargo.toml" <<'EOF_INNER'
[package]
name = "fixture"
version = "0.1.0"
EOF_INNER
  cat >"$dir/README.md" <<'EOF_INNER'
# so-context
intro line
second line
EOF_INNER
  cat >"$dir/src/main.rs" <<'EOF_INNER'
fn main() {
    // TODO one
}
EOF_INNER
  cat >"$dir/src/lib.rs" <<'EOF_INNER'
// TODO two
EOF_INNER
  cat >"$dir/src/shell/mod.rs" <<'EOF_INNER'
pub mod mod_file;
EOF_INNER
  set_setup_paths "$dir" "$dir"
}

setup_node_fixture() {
  local dir="$fixture_root/node-fixture"
  mkdir -p "$dir/scripts"
  cat >"$dir/package.json" <<'EOF_INNER'
{
  "name": "shell-audit-node-fixture",
  "private": true,
  "scripts": {
    "demo": "node scripts/demo.js"
  }
}
EOF_INNER
  cat >"$dir/scripts/demo.js" <<'EOF_INNER'
console.log("Creating an optimized production build...");
console.log("✓ Build completed");
EOF_INNER
  set_setup_paths "$dir" "$dir"
}

setup_make_fixture() {
  local dir="$fixture_root/make-fixture"
  mkdir -p "$dir"
  cat >"$dir/Makefile" <<'EOF_INNER'
test:
	@echo "Nothing to be done for \`test'."
EOF_INNER
  set_setup_paths "$dir" "$dir"
}

setup_rust_fixture() {
  local dir="$fixture_root/rust-fixture"
  mkdir -p "$dir/src" "$dir/tests"
  cat >"$dir/Cargo.toml" <<'EOF_INNER'
[package]
name = "shell_audit_fixture"
version = "0.1.0"
edition = "2024"
EOF_INNER
  cat >"$dir/src/lib.rs" <<'EOF_INNER'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
EOF_INNER
  cat >"$dir/src/main.rs" <<'EOF_INNER'
fn main() {
    println!("{}", shell_audit_fixture::add(20, 22));
}
EOF_INNER
  cat >"$dir/tests/basic.rs" <<'EOF_INNER'
use shell_audit_fixture::add;

#[test]
fn adds_numbers() {
    assert_eq!(add(2, 3), 5);
}
EOF_INNER
  set_setup_paths "$dir" "$dir"
}

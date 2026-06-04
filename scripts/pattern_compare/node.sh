#!/usr/bin/env bash
set -euo pipefail

setup_node_workspace() {
  local base="$1"
  mkdir -p "$base/repo/scripts"
  cat >"$base/repo/package.json" <<'EOF'
{
  "name": "pattern-compare-node-fixture",
  "private": true,
  "scripts": {
    "demo": "node scripts/demo.js"
  }
}
EOF
  cat >"$base/repo/scripts/demo.js" <<'EOF'
console.log("Creating an optimized production build...");
console.log("✓ Build completed");
EOF
}

register_node_cases() {
  capture_rtk_dedicated_case "npm-build" "npm build" "node.npm" \
    "npm run demo against a local fixture package." \
    setup_node_workspace "repo" "npm run demo" "npm" \
    npm run demo
  capture_rtk_dedicated_case "pnpm-run" "pnpm run demo" "node.pnpm" \
    "pnpm run demo against a local fixture package." \
    setup_node_workspace "repo" "pnpm run demo" "pnpm" \
    pnpm run demo
  capture_rtk_dedicated_case "npx-version" "npx version" "node.npx" \
    "npx version output from the real installed binary." \
    setup_node_workspace "repo" "npx --version" "npx" \
    npx --version

  if command_exists yarn; then
    capture_rtk_summary_case "yarn-run" "yarn run demo" "node.yarn" \
      "yarn run demo against a local fixture package. RTK has no dedicated yarn wrapper." \
      setup_node_workspace "repo" "yarn run demo" \
      yarn run demo
  else
    write_skip_case \
      "$output_root/patterns/node.yarn/yarn-run" \
      "yarn run demo" \
      "node.yarn" \
      "rtk-summary" \
      "yarn run demo" \
      "yarn is not installed in this environment." \
      "native yarn binary not available"
  fi

  if command_exists bun; then
    capture_rtk_summary_case "bun-run" "bun run demo" "node.bun" \
      "bun run demo against a local fixture package. RTK has no dedicated bun wrapper." \
      setup_node_workspace "repo" "bun run demo" \
      bun run demo
  else
    write_skip_case \
      "$output_root/patterns/node.bun/bun-run" \
      "bun run demo" \
      "node.bun" \
      "rtk-summary" \
      "bun run demo" \
      "bun is not installed in this environment." \
      "native bun binary not available"
  fi
}

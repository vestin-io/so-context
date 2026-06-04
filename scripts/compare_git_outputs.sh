#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Backward-compatible wrapper. The compare suite now covers all pattern families,
# not just git. Keep git-only reference output under `.so-context/reference-git-compare`
# when no explicit path is given.
if [ "$#" -eq 0 ]; then
  exec "$repo_root/scripts/compare_pattern_outputs.sh" "$repo_root/.so-context/reference-git-compare"
else
  exec "$repo_root/scripts/compare_pattern_outputs.sh" "$@"
fi

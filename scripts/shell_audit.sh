#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script_dir="$repo_root/scripts"
bin="$repo_root/target/debug/so-context"
db="${HOME}/.local/share/so-context/events.db"
output_path="$repo_root/.so-context/audits/shell-audit-latest.json"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/so-context-shell-audit.XXXXXX")"
fixture_root="$tmp_root/fixtures"
runner_root="$tmp_root/runners"
capture_root="$tmp_root/captures"
cases_jsonl="$tmp_root/cases.jsonl"
docker_live_project="so-context-audit-live"
docker_live_logger="so-context-audit-logger"
docker_live_worker="so-context-audit-worker"
k8s_namespace="so-context-audit"
k8s_pod_name="log-demo"
k8s_service_name="log-demo"
suite="extended"
run_id_env="SO_CONTEXT_SHELL_RUN_ID"

mkdir -p "$fixture_root" "$runner_root" "$capture_root"

source "$script_dir/shell_audit_fixtures.sh"
source "$script_dir/shell_audit_fixtures_git.sh"
source "$script_dir/shell_audit_fixtures_runtime.sh"
source "$script_dir/shell_audit_suites.sh"

cleanup() {
  docker compose -p "$docker_live_project" -f "$fixture_root/docker-runtime/compose.yml" down >/dev/null 2>&1 || true
  docker rm -f "$docker_live_logger" "$docker_live_worker" >/dev/null 2>&1 || true
  kubectl delete namespace "$k8s_namespace" --ignore-not-found >/dev/null 2>&1 || true
  rm -rf "$tmp_root"
}
trap cleanup EXIT

usage() {
  cat <<'EOF_USAGE'
Usage:
  bash scripts/shell_audit.sh
  bash scripts/shell_audit.sh --suite core
  bash scripts/shell_audit.sh --suite extended
  bash scripts/shell_audit.sh --output /path/to/report.json
  bash scripts/shell_audit.sh --stdout

Runs a fixed shell audit suite, compares native command output with `so-context shell`,
and writes a JSON report. No RTK commands are used.
EOF_USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --suite)
      suite="$2"
      shift 2
      ;;
    --output)
      output_path="$2"
      shift 2
      ;;
    --stdout)
      output_path=""
      shift
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$suite" != "core" && "$suite" != "extended" ]]; then
  echo "invalid suite: $suite" >&2
  exit 1
fi

ensure_binary() {
  if [[ ! -x "$bin" ]]; then
    (cd "$repo_root" && cargo build --quiet)
  fi
}

reset_metrics() {
  if [[ -f "$db" ]]; then
    sqlite3 "$db" "DELETE FROM shell_runs;"
  fi
}

run_capture() {
  local cwd="$1"
  local prefix="$2"
  shift 2
  local stdout_file="$capture_root/$prefix.stdout"
  local stderr_file="$capture_root/$prefix.stderr"
  local exit_code=0
  (
    cd "$cwd"
    "$@" >"$stdout_file" 2>"$stderr_file"
  ) || exit_code=$?
  printf '%s\n%s\n%s\n' "$stdout_file" "$stderr_file" "$exit_code"
}

copy_fixture_tree() {
  local source_root="$1"
  local destination_root="$2"
  rm -rf "$destination_root"
  mkdir -p "$destination_root"
  cp -R "$source_root"/. "$destination_root"
}

latest_metrics_json() {
  local run_id="$1"
  sqlite3 "$db" -json "
    SELECT
      run_id,
      family,
      render_mode,
      raw_stdout_bytes + raw_stderr_bytes AS raw_bytes,
      compressed_bytes
    FROM shell_runs
    WHERE run_id = '$run_id'
    LIMIT 1;
  "
}

make_run_id() {
  local name="$1"
  local safe_name
  safe_name="$(printf '%s' "$name" | tr -c '[:alnum:]._' '-')"
  printf '%s-%s-%s' "$safe_name" "$$" "$(date +%s%N)"
}

reviewed_ok() {
  local family="$1"
  local render_mode="$2"
  local saved_pct="$3"
  case "$family" in
    rust.cargo-test|generic.grep|generic.env|docker.inspect|k8s.kubectl-describe)
      awk "BEGIN { exit !($saved_pct > 90.0) }"
      ;;
    rust.cargo-check|build.make|docker.compose|docker.logs|k8s.kubectl-logs)
      [[ "$render_mode" == "raw_fallback" ]]
      ;;
    *)
      return 1
      ;;
  esac
}

acceptable_raw_fallback() {
  local raw_bytes="$1"
  [[ "$raw_bytes" -le 160 ]]
}

case_json() {
  local name="$1"
  local cwd="$2"
  local clone_root="$3"
  shift 3
  local -a cmd=("$@")

  local native_cwd="$cwd"
  local shell_cwd="$cwd"
  if [[ "$clone_root" != "-" ]]; then
    local rel_cwd native_root shell_root
    rel_cwd="${cwd#$clone_root}"
    rel_cwd="${rel_cwd#/}"
    native_root="$runner_root/${name}.native-root"
    shell_root="$runner_root/${name}.shell-root"
    copy_fixture_tree "$clone_root" "$native_root"
    copy_fixture_tree "$clone_root" "$shell_root"
    native_cwd="$native_root"
    shell_cwd="$shell_root"
    if [[ -n "$rel_cwd" ]]; then
      native_cwd="$native_root/$rel_cwd"
      shell_cwd="$shell_root/$rel_cwd"
    fi
  fi

  local native_meta shell_meta run_id
  run_id="$(make_run_id "$name")"
  native_meta="$(run_capture "$native_cwd" "$name.native" "${cmd[@]}")"
  shell_meta="$(
    run_capture \
      "$shell_cwd" \
      "$name.shell" \
      env "$run_id_env=$run_id" "$bin" shell -- "${cmd[@]}"
  )"

  local native_stdout native_stderr native_exit shell_stdout shell_stderr shell_exit
  native_stdout="$(printf '%s\n' "$native_meta" | sed -n '1p')"
  native_stderr="$(printf '%s\n' "$native_meta" | sed -n '2p')"
  native_exit="$(printf '%s\n' "$native_meta" | sed -n '3p')"
  shell_stdout="$(printf '%s\n' "$shell_meta" | sed -n '1p')"
  shell_stderr="$(printf '%s\n' "$shell_meta" | sed -n '2p')"
  shell_exit="$(printf '%s\n' "$shell_meta" | sed -n '3p')"

  local metrics_json metrics family render_mode raw_bytes compressed_bytes saved_pct
  metrics_json="$(latest_metrics_json "$run_id")"
  metrics="$(jq '.[0]' <<<"$metrics_json")"
  family="$(jq -r '.family' <<<"$metrics")"
  render_mode="$(jq -r '.render_mode' <<<"$metrics")"
  raw_bytes="$(jq -r '.raw_bytes' <<<"$metrics")"
  compressed_bytes="$(jq -r '.compressed_bytes' <<<"$metrics")"
  saved_pct="$(awk "BEGIN { if ($raw_bytes > 0) printf \"%.1f\", (1 - ($compressed_bytes / $raw_bytes)) * 100; else print \"0.0\" }")"

  local status="ok"
  local reasons='[]'
  if [[ "$native_exit" != "$shell_exit" ]]; then
    status="suspicious"
    reasons="$(jq -c '. + ["exit_code_mismatch"]' <<<"$reasons")"
  fi
  if [[ "$family" == "unknown" ]]; then
    status="suspicious"
    reasons="$(jq -c '. + ["unknown_pattern"]' <<<"$reasons")"
  fi
  if [[ "$render_mode" == "compressed" ]] && awk "BEGIN { exit !($saved_pct < 10.0 || $saved_pct > 90.0) }"; then
    if ! reviewed_ok "$family" "$render_mode" "$saved_pct"; then
      status="suspicious"
      reasons="$(jq -c '. + ["saved_pct_out_of_range"]' <<<"$reasons")"
    fi
  fi
  if [[ "$render_mode" == "raw_fallback" ]]; then
    if ! reviewed_ok "$family" "$render_mode" "$saved_pct" && ! acceptable_raw_fallback "$raw_bytes"; then
      status="suspicious"
      reasons="$(jq -c '. + ["raw_fallback"]' <<<"$reasons")"
    fi
  fi
  if [[ "$compressed_bytes" -gt "$raw_bytes" ]]; then
    status="suspicious"
    reasons="$(jq -c '. + ["compressed_larger_than_raw"]' <<<"$reasons")"
  fi

  jq -n \
    --arg name "$name" \
    --arg command "${cmd[*]}" \
    --arg cwd "$cwd" \
    --arg family "$family" \
    --arg render_mode "$render_mode" \
    --arg status "$status" \
    --argjson raw_bytes "$raw_bytes" \
    --argjson compressed_bytes "$compressed_bytes" \
    --argjson saved_pct "$saved_pct" \
    --argjson native_exit_code "$native_exit" \
    --argjson shell_exit_code "$shell_exit" \
    --argjson suspicious_reasons "$reasons" \
    --rawfile raw_stdout "$native_stdout" \
    --rawfile raw_stderr "$native_stderr" \
    --rawfile shell_stdout "$shell_stdout" \
    --rawfile shell_stderr "$shell_stderr" \
    '{
      name: $name,
      command: $command,
      cwd: $cwd,
      family: $family,
      status: $status,
      render_mode: $render_mode,
      raw_bytes: $raw_bytes,
      compressed_bytes: $compressed_bytes,
      saved_pct: $saved_pct,
      native_exit_code: $native_exit_code,
      shell_exit_code: $shell_exit_code,
      suspicious_reasons: $suspicious_reasons,
      raw_stdout: $raw_stdout,
      raw_stderr: $raw_stderr,
      shell_stdout: $shell_stdout,
      shell_stderr: $shell_stderr
    }'
}

append_case() {
  local name="$1"
  local cwd="$2"
  shift 2
  case_json "$name" "$cwd" "-" "$@" >>"$cases_jsonl"
}

append_setup_case() {
  local name="$1"
  local setup="$2"
  shift 2
  clear_setup_paths
  if ! "$setup" >/dev/null 2>&1; then
    append_skipped_case "$name" "${*}" "fixture unavailable"
    return
  fi
  case_json "$name" "$SETUP_CWD" "$SETUP_ROOT" "$@" >>"$cases_jsonl"
}

append_skipped_case() {
  jq -n \
    --arg name "$1" \
    --arg command "$2" \
    --arg reason "$3" \
    '{name: $name, command: $command, status: "skipped", skip_reason: $reason}' >>"$cases_jsonl"
}

write_report() {
  local report_json
  report_json="$(jq -s '
    . as $cases
    | {
        generated_at: now | todate,
        suite: "shell-audit-v1-'$suite'",
        repo_root: "'$repo_root'",
        summary: {
          total_cases: ($cases | length),
          ok_cases: ($cases | map(select(.status == "ok")) | length),
          suspicious_cases: ($cases | map(select(.status == "suspicious")) | length),
          skipped_cases: ($cases | map(select(.status == "skipped")) | length)
        },
        suspicious_case_names: ($cases | map(select(.status == "suspicious") | .name)),
        cases: $cases
      }' "$cases_jsonl")"

  if [[ -n "$output_path" ]]; then
    mkdir -p "$(dirname "$output_path")"
    printf '%s\n' "$report_json" >"$output_path"
    printf 'wrote %s\n' "$output_path"
  else
    printf '%s\n' "$report_json"
  fi
}

main() {
  ensure_binary
  reset_metrics
  : >"$cases_jsonl"
  append_core_cases
  if [[ "$suite" == "extended" ]]; then
    append_extended_cases
  fi
  write_report
}

main

#!/usr/bin/env bash
set -euo pipefail

setup_make_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/Makefile" <<'EOF'
test:
	@echo "Nothing to be done for \`test'."
EOF
}

register_build_cases() {
  if command_exists make; then
    capture_rtk_summary_case "make-test" "Make target" "build.make" \
      "Make reports that the target is already up to date. RTK has no dedicated make wrapper." \
      setup_make_workspace "repo" "make test" \
      make test
  else
    write_skip_case \
      "$output_root/patterns/build.make/make-test" \
      "Make target" \
      "build.make" \
      "rtk-summary" \
      "make test" \
      "make is not installed in this environment." \
      "native make binary not available"
  fi

  write_skip_case "$output_root/patterns/build.dotnet-build/dotnet-build" ".NET build" "build.dotnet-build" "rtk-dotnet" "dotnet build" "Skipped for now: local dotnet fixture restore hits nuget.org, which is unavailable in the current sandbox." "nuget network dependency blocked"
  write_skip_case "$output_root/patterns/build.dotnet-restore/dotnet-restore" ".NET restore" "build.dotnet-restore" "rtk-dotnet" "dotnet restore" "Skipped for now: local dotnet fixture restore hits nuget.org, which is unavailable in the current sandbox." "nuget network dependency blocked"
  write_skip_case "$output_root/patterns/build.dotnet-format/dotnet-format" ".NET format" "build.dotnet-format" "rtk-dotnet" "dotnet format" "Skipped for now: local dotnet fixture restore hits nuget.org, which is unavailable in the current sandbox." "nuget network dependency blocked"
  write_skip_case "$output_root/patterns/build.dotnet-test/dotnet-test" ".NET test" "build.dotnet-test" "rtk-dotnet" "dotnet test" "Skipped for now: realistic dotnet test fixtures need NuGet restore, which is unavailable in the current sandbox." "nuget network dependency blocked"
  write_skip_case "$output_root/patterns/build.tsc/tsc-errors" "TypeScript compile" "build.tsc" "rtk-tsc" "tsc --noEmit" "Skipped for now: tsc is not installed in this environment." "native tsc binary not available"
  write_skip_case "$output_root/patterns/build.next/next-build" "Next.js build" "build.next" "rtk-next" "next build" "Skipped for now: next is not installed in this environment." "native next binary not available"
  write_skip_case "$output_root/patterns/build.vite/vite-build" "Vite build" "build.vite" "rtk-summary" "vite build" "Skipped for now: vite is not installed in this environment." "native vite binary not available"
  write_skip_case "$output_root/patterns/build.gradle/gradle-build" "Gradle build" "build.gradle" "rtk-summary" "gradlew build" "Skipped for now: gradlew is not installed in this environment." "native gradlew binary not available"
  write_skip_case "$output_root/patterns/build.maven/maven-test" "Maven build" "build.maven" "rtk-summary" "mvn test" "Skipped for now: mvn is not installed in this environment." "native mvn binary not available"
  write_skip_case "$output_root/patterns/build.cmake/cmake-build" "CMake build" "build.cmake" "rtk-summary" "cmake --build build" "Skipped for now: cmake is not installed in this environment." "native cmake binary not available"
}

#!/usr/bin/env bash
set -euo pipefail

docker_daemon_ready() {
  docker info >/dev/null 2>&1
}

docker_live_project="so-context-pattern-compare-live"
docker_live_logger="so-context-pattern-compare-logger"
docker_live_worker="so-context-pattern-compare-worker"
docker_live_filter="label=com.docker.compose.project=$docker_live_project"

setup_docker_compose_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/compose.yml" <<'EOF'
services:
  web:
    image: nginx:latest
    ports:
      - "3000:3000"
EOF
}

setup_docker_compose_multi_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/compose.yml" <<'EOF'
services:
  web:
    image: nginx:latest
  worker:
    image: busybox:latest
EOF
}

setup_docker_image_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/Dockerfile" <<'EOF'
FROM scratch
COPY hello.txt /hello.txt
LABEL org.opencontainers.image.title="so-context-pattern-compare"
EOF
  printf '%s\n' "hello" >"$base/repo/hello.txt"
}

setup_docker_runtime_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/compose.yml" <<EOF
services:
  logger:
    image: busybox:latest
    container_name: $docker_live_logger
    command: ["sh", "-c", "echo booting; echo ERROR failed to connect; sleep 300"]
  worker:
    image: busybox:latest
    container_name: $docker_live_worker
    command: ["sh", "-c", "sleep 300"]
EOF
}

docker_cleanup_live_fixture() {
  local fixture_dir="${docker_live_dir:-}"
  if [ -z "$fixture_dir" ] || [ ! -f "$fixture_dir/compose.yml" ]; then
    docker rm -f "$docker_live_logger" "$docker_live_worker" >/dev/null 2>&1 || true
    return
  fi
  docker compose -p "$docker_live_project" -f "$fixture_dir/compose.yml" down >/dev/null 2>&1 || true
  docker rm -f "$docker_live_logger" "$docker_live_worker" >/dev/null 2>&1 || true
}

prepare_docker_runtime_fixture() {
  docker_live_dir="$work_root/docker-live/repo"
  local base="$work_root/docker-live-base"
  rm -rf "$base" "$docker_live_dir"
  mkdir -p "$base"
  setup_docker_runtime_workspace "$base"
  mkdir -p "$(dirname "$docker_live_dir")"
  cp -R "$base/repo" "$docker_live_dir"

  docker_cleanup_live_fixture
  docker image inspect busybox:latest >/dev/null 2>&1 || docker pull busybox:latest >/dev/null
  docker compose -p "$docker_live_project" -f "$docker_live_dir/compose.yml" up -d >/dev/null
}

build_fixture_image() {
  local repo_dir="$1"
  docker image rm -f so-context-pattern-compare:latest >/dev/null 2>&1 || true
  docker build -t so-context-pattern-compare:latest "$repo_dir" >/dev/null
}

setup_docker_images_fixture() {
  local base="$1"
  setup_docker_image_workspace "$base"
  build_fixture_image "$base/repo"
}

register_docker_cases() {
  if ! command_exists docker; then
    write_skip_case "$output_root/patterns/docker.compose/compose-config" "Docker compose config" "docker.compose" "rtk-docker" "docker compose config" "docker is not installed in this environment." "native docker binary not available"
    write_skip_case "$output_root/patterns/docker.images/images-filtered" "Docker images" "docker.images" "rtk-docker" "docker images --filter reference=so-context-pattern-compare" "docker is not installed in this environment." "native docker binary not available"
    write_skip_case "$output_root/patterns/docker.build/build-fixture" "Docker build" "docker.build" "rtk-summary" "docker build ." "docker is not installed in this environment." "native docker binary not available"
    write_skip_case "$output_root/patterns/docker.inspect/inspect-image" "Docker inspect" "docker.inspect" "rtk-summary" "docker inspect so-context-pattern-compare:latest" "docker is not installed in this environment." "native docker binary not available"
    write_skip_case "$output_root/patterns/docker.ps/ps-live" "Docker ps" "docker.ps" "rtk-docker" "docker ps" "Skipped for now: no stable fixture for a running container set." "requires live container state"
    write_skip_case "$output_root/patterns/docker.logs/logs-live" "Docker logs" "docker.logs" "rtk-docker" "docker logs fixture" "Skipped for now: no stable fixture for container logs." "requires live container state"
    write_skip_case "$output_root/patterns/docker.pull/pull-live" "Docker pull" "docker.pull" "rtk-summary" "docker pull nginx:latest" "Skipped for now: requires network/image registry." "requires external registry access"
    return
  fi

  capture_rtk_dedicated_case "compose-config" "Docker compose config" "docker.compose" \
    "docker compose config on a local compose file." \
    setup_docker_compose_workspace "repo" "docker compose config" "docker" \
    docker compose config
  capture_rtk_dedicated_case "compose-config-services" "Docker compose services" "docker.compose" \
    "docker compose config --services on a local compose file." \
    setup_docker_compose_multi_workspace "repo" "docker compose config --services" "docker" \
    docker compose config --services
  capture_rtk_dedicated_case "compose-config-images" "Docker compose images" "docker.compose" \
    "docker compose config --images on a local compose file." \
    setup_docker_compose_multi_workspace "repo" "docker compose config --images" "docker" \
    docker compose config --images

  if docker_daemon_ready; then
    if prepare_docker_runtime_fixture; then
      capture_rtk_dedicated_case "ps-live" "Docker ps" "docker.ps" \
        "docker ps filtered to the live compare project containers." \
        setup_docker_runtime_workspace "repo" "docker ps --filter $docker_live_filter" "docker" \
        docker ps --filter "$docker_live_filter"
      capture_rtk_dedicated_case "logs-live" "Docker logs" "docker.logs" \
        "docker logs for the live compare logger container." \
        setup_docker_runtime_workspace "repo" "docker logs $docker_live_logger" "docker" \
        docker logs "$docker_live_logger"
    else
      write_skip_case "$output_root/patterns/docker.ps/ps-live" "Docker ps" "docker.ps" "rtk-docker" "docker ps --filter $docker_live_filter" "Skipped for now: the live docker runtime fixture could not be started." "runtime fixture unavailable"
      write_skip_case "$output_root/patterns/docker.logs/logs-live" "Docker logs" "docker.logs" "rtk-docker" "docker logs $docker_live_logger" "Skipped for now: the live docker runtime fixture could not be started." "runtime fixture unavailable"
    fi
    capture_rtk_dedicated_case "images-filtered" "Docker images" "docker.images" \
      "docker images filtered to a locally built scratch image." \
      setup_docker_images_fixture "repo" "docker images --filter reference=so-context-pattern-compare" "docker" \
      docker images --filter reference=so-context-pattern-compare
    capture_rtk_summary_case "build-fixture" "Docker build" "docker.build" \
      "docker build on a local scratch-based Dockerfile. RTK has no dedicated build wrapper." \
      setup_docker_image_workspace "repo" "docker build ." \
      docker build .
    capture_rtk_summary_case "inspect-image" "Docker inspect" "docker.inspect" \
      "docker inspect on a locally built image. RTK has no dedicated inspect wrapper." \
      setup_docker_images_fixture "repo" "docker inspect so-context-pattern-compare:latest" \
      docker inspect so-context-pattern-compare:latest
  else
    write_skip_case "$output_root/patterns/docker.images/images-filtered" "Docker images" "docker.images" "rtk-docker" "docker images --filter reference=so-context-pattern-compare" "Skipped: docker daemon is not available." "docker daemon unavailable"
    write_skip_case "$output_root/patterns/docker.build/build-fixture" "Docker build" "docker.build" "rtk-summary" "docker build ." "Skipped: docker daemon is not available." "docker daemon unavailable"
    write_skip_case "$output_root/patterns/docker.inspect/inspect-image" "Docker inspect" "docker.inspect" "rtk-summary" "docker inspect so-context-pattern-compare:latest" "Skipped: docker daemon is not available." "docker daemon unavailable"
    write_skip_case "$output_root/patterns/docker.ps/ps-live" "Docker ps" "docker.ps" "rtk-docker" "docker ps --filter $docker_live_filter" "Skipped: docker daemon is not available." "docker daemon unavailable"
    write_skip_case "$output_root/patterns/docker.logs/logs-live" "Docker logs" "docker.logs" "rtk-docker" "docker logs $docker_live_logger" "Skipped: docker daemon is not available." "docker daemon unavailable"
  fi

  write_skip_case "$output_root/patterns/docker.pull/pull-live" "Docker pull" "docker.pull" "rtk-summary" "docker pull nginx:latest" "Skipped for now: requires network/image registry." "requires external registry access"
}

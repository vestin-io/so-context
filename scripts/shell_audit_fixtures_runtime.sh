#!/usr/bin/env bash

setup_docker_compose_fixture() {
  local dir="$fixture_root/docker-compose"
  mkdir -p "$dir"
  cat >"$dir/compose.yml" <<'EOF_INNER'
services:
  web:
    image: nginx:latest
  worker:
    image: busybox:latest
EOF_INNER
  set_setup_paths "$dir" "$dir"
}

setup_docker_image_fixture() {
  local dir="$fixture_root/docker-image"
  mkdir -p "$dir"
  cat >"$dir/Dockerfile" <<'EOF_INNER'
FROM scratch
COPY hello.txt /hello.txt
LABEL org.opencontainers.image.title="so-context-shell-audit"
EOF_INNER
  printf '%s\n' "hello" >"$dir/hello.txt"
  docker image rm -f so-context-shell-audit:latest >/dev/null 2>&1 || true
  docker build -t so-context-shell-audit:latest "$dir" >/dev/null
  set_setup_paths "$dir" "$dir"
}

setup_docker_runtime_fixture() {
  local dir="$fixture_root/docker-runtime"
  mkdir -p "$dir"
  cat >"$dir/compose.yml" <<EOF_INNER
services:
  logger:
    image: busybox:latest
    container_name: $docker_live_logger
    command: ["sh", "-c", "echo booting; echo ERROR failed to connect; sleep 300"]
  worker:
    image: busybox:latest
    container_name: $docker_live_worker
    command: ["sh", "-c", "sleep 300"]
EOF_INNER
  docker image inspect busybox:latest >/dev/null 2>&1 || docker pull busybox:latest >/dev/null
  docker compose -p "$docker_live_project" -f "$dir/compose.yml" up -d >/dev/null
  set_setup_paths "$dir" "$repo_root"
}

setup_k8s_fixture() {
  local dir="$fixture_root/k8s-fixture"
  mkdir -p "$dir"
  cat >"$dir/fixture.yaml" <<EOF_INNER
apiVersion: v1
kind: Pod
metadata:
  name: $k8s_pod_name
  namespace: $k8s_namespace
  labels:
    app: $k8s_service_name
spec:
  restartPolicy: Never
  containers:
    - name: app
      image: busybox:latest
      command:
        - sh
        - -c
        - |
          echo booting
          echo ERROR backend unavailable
          sleep 300
---
apiVersion: v1
kind: Service
metadata:
  name: $k8s_service_name
  namespace: $k8s_namespace
spec:
  selector:
    app: $k8s_service_name
  ports:
    - port: 80
      targetPort: 80
EOF_INNER
  kubectl create namespace "$k8s_namespace" >/dev/null 2>&1 || true
  kubectl apply -f "$dir/fixture.yaml" >/dev/null
  kubectl wait --namespace "$k8s_namespace" --for=condition=Ready "pod/$k8s_pod_name" --timeout=120s >/dev/null
  set_setup_paths "$dir" "$repo_root"
}

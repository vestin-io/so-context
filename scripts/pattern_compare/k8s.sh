#!/usr/bin/env bash
set -euo pipefail

k8s_namespace="so-context-compare"
k8s_pod_name="log-demo"
k8s_service_name="log-demo"
k8s_manifest_file="k8s-fixture.yaml"

setup_k8s_workspace() {
  local base="$1"
  mkdir -p "$base/repo"
  cat >"$base/repo/$k8s_manifest_file" <<EOF
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
EOF
}

k8s_cleanup_live_fixture() {
  kubectl delete namespace "$k8s_namespace" --ignore-not-found >/dev/null 2>&1 || true
}

prepare_k8s_live_fixture() {
  local base="$work_root/k8s-live-base"
  local repo_dir="$work_root/k8s-live/repo"
  rm -rf "$base" "$repo_dir"
  mkdir -p "$base" "$(dirname "$repo_dir")"
  setup_k8s_workspace "$base"
  cp -R "$base/repo" "$repo_dir"

  k8s_cleanup_live_fixture
  kubectl create namespace "$k8s_namespace" >/dev/null
  kubectl apply -f "$repo_dir/$k8s_manifest_file" >/dev/null
  kubectl wait --namespace "$k8s_namespace" --for=condition=Ready "pod/$k8s_pod_name" --timeout=120s >/dev/null
}

register_k8s_cases() {
  if ! command_exists kubectl; then
    write_skip_case "$output_root/patterns/k8s.kubectl-pods/get-pods" "kubectl get pods" "k8s.kubectl-pods" "rtk-kubectl" "kubectl get pods -n $k8s_namespace" "kubectl is not installed in this environment." "native kubectl binary not available"
    write_skip_case "$output_root/patterns/k8s.kubectl-services/get-services" "kubectl get services" "k8s.kubectl-services" "rtk-kubectl" "kubectl get services -n $k8s_namespace" "kubectl is not installed in this environment." "native kubectl binary not available"
    write_skip_case "$output_root/patterns/k8s.kubectl-logs/logs-pod" "kubectl logs" "k8s.kubectl-logs" "rtk-kubectl" "kubectl logs $k8s_pod_name -n $k8s_namespace" "kubectl is not installed in this environment." "native kubectl binary not available"
    write_skip_case "$output_root/patterns/k8s.kubectl-describe/describe-pod" "kubectl describe" "k8s.kubectl-describe" "rtk-summary" "kubectl describe pod $k8s_pod_name -n $k8s_namespace" "kubectl is not installed in this environment." "native kubectl binary not available"
    write_skip_case "$output_root/patterns/k8s.kubectl-apply/apply-manifest" "kubectl apply" "k8s.kubectl-apply" "rtk-summary" "kubectl apply -f $k8s_manifest_file -n $k8s_namespace" "kubectl is not installed in this environment." "native kubectl binary not available"
    return
  fi

  if prepare_k8s_live_fixture; then
    capture_rtk_dedicated_case "get-pods" "kubectl get pods" "k8s.kubectl-pods" \
      "Live pod listing from the local kind fixture namespace." \
      setup_k8s_workspace "repo" "kubectl get pods -n $k8s_namespace" "kubectl" \
      kubectl get pods -n "$k8s_namespace"
    capture_rtk_dedicated_case "get-services" "kubectl get services" "k8s.kubectl-services" \
      "Live service listing from the local kind fixture namespace." \
      setup_k8s_workspace "repo" "kubectl get services -n $k8s_namespace" "kubectl" \
      kubectl get services -n "$k8s_namespace"
    capture_rtk_dedicated_case "logs-pod" "kubectl logs" "k8s.kubectl-logs" \
      "Live pod logs from the local kind fixture pod." \
      setup_k8s_workspace "repo" "kubectl logs $k8s_pod_name -n $k8s_namespace" "kubectl" \
      kubectl logs "$k8s_pod_name" -n "$k8s_namespace"
    capture_rtk_summary_case "describe-pod" "kubectl describe" "k8s.kubectl-describe" \
      "Live pod describe from the local kind fixture pod. RTK has no dedicated describe wrapper." \
      setup_k8s_workspace "repo" "kubectl describe pod $k8s_pod_name -n $k8s_namespace" \
      kubectl describe pod "$k8s_pod_name" -n "$k8s_namespace"
    capture_rtk_summary_case "apply-manifest" "kubectl apply" "k8s.kubectl-apply" \
      "Live apply against the local kind fixture manifest. RTK has no dedicated apply wrapper." \
      setup_k8s_workspace "repo" "kubectl apply -f $k8s_manifest_file -n $k8s_namespace" \
      kubectl apply -f "$k8s_manifest_file" -n "$k8s_namespace"
  else
    write_skip_case "$output_root/patterns/k8s.kubectl-pods/get-pods" "kubectl get pods" "k8s.kubectl-pods" "rtk-kubectl" "kubectl get pods -n $k8s_namespace" "Skipped for now: local kind fixture could not be prepared." "kind fixture unavailable"
    write_skip_case "$output_root/patterns/k8s.kubectl-services/get-services" "kubectl get services" "k8s.kubectl-services" "rtk-kubectl" "kubectl get services -n $k8s_namespace" "Skipped for now: local kind fixture could not be prepared." "kind fixture unavailable"
    write_skip_case "$output_root/patterns/k8s.kubectl-logs/logs-pod" "kubectl logs" "k8s.kubectl-logs" "rtk-kubectl" "kubectl logs $k8s_pod_name -n $k8s_namespace" "Skipped for now: local kind fixture could not be prepared." "kind fixture unavailable"
    write_skip_case "$output_root/patterns/k8s.kubectl-describe/describe-pod" "kubectl describe" "k8s.kubectl-describe" "rtk-summary" "kubectl describe pod $k8s_pod_name -n $k8s_namespace" "Skipped for now: local kind fixture could not be prepared." "kind fixture unavailable"
    write_skip_case "$output_root/patterns/k8s.kubectl-apply/apply-manifest" "kubectl apply" "k8s.kubectl-apply" "rtk-summary" "kubectl apply -f $k8s_manifest_file -n $k8s_namespace" "Skipped for now: local kind fixture could not be prepared." "kind fixture unavailable"
  fi
}

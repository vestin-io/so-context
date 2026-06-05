use super::*;
use crate::shell::types::ShellInvocation;

fn summarize_case(result: &ShellResult) -> CompressionSummary {
    summarize(result, super::super::classify_only(result))
}

#[test]
fn summarizes_kubectl_pods() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["kubectl".into(), "get".into(), "pods".into()]),
        stdout: "NAME READY STATUS RESTARTS AGE\napi-0 1/1 Running 1 5m\njob-0 0/1 Pending 0 1m\n"
            .into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlPods);
    assert_eq!(
        summary.summary,
        "2 pods | running=1 pending=1 failed=0 restarts=1"
    );
}

#[test]
fn summarizes_kubectl_services() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["kubectl".into(), "get".into(), "services".into()]),
        stdout: "NAME TYPE CLUSTER-IP EXTERNAL-IP PORT(S) AGE\napi ClusterIP 10.0.0.1 <none> 80/TCP 1d\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlServices);
    assert_eq!(summary.summary, "1 services");
    assert_eq!(summary.details[0], "api ClusterIP [80/TCP]");
}

#[test]
fn summarizes_kubectl_logs() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["kubectl".into(), "logs".into(), "api-0".into()]),
        stdout: "booting\nERROR database unavailable\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlLogs);
    assert_eq!(summary.summary, "logs for api-0: 1 errors, 0 warnings");
    assert_eq!(summary.details[0], "booting");
    assert_eq!(summary.details[1], "ERROR database unavailable");
}

#[test]
fn summarizes_kubectl_logs_without_issues() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["kubectl".into(), "logs".into(), "api-0".into()]),
        stdout: "booting\nready\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "logs for api-0: 2 lines");
    assert_eq!(summary.details[0], "booting");
    assert_eq!(summary.details[1], "ready");
}

#[test]
fn summarizes_kubectl_describe() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "kubectl".into(),
            "describe".into(),
            "pod".into(),
            "api-0".into(),
        ]),
        stdout: "Name: api-0\nNamespace: default\nStatus: Running\nNode: kind-control-plane/172.23.0.2\nImage: api:latest\nEvents:\n  Type    Reason   Age   From      Message\n  ----    ------   ----  ----      -------\n  Normal  Started  2s    kubelet   Container started\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlDescribe);
    assert_eq!(summary.summary, "api-0 Running");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Node: kind-control-plane"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Normal Started 2s kubelet Container started"))
    );
}

#[test]
fn summarizes_kubectl_apply() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "kubectl".into(),
            "apply".into(),
            "-f".into(),
            "deploy.yaml".into(),
        ]),
        stdout: "deployment.apps/api configured\nservice/api unchanged\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlApply);
    assert_eq!(summary.summary, "kubectl apply: 1 configured, 1 unchanged");
}

#[test]
fn classifies_get_pods_with_namespace_flag_before_resource() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "kubectl".into(),
            "get".into(),
            "-n".into(),
            "default".into(),
            "pods".into(),
        ]),
        stdout: "NAME READY STATUS RESTARTS AGE\napi-0 1/1 Running 0 5m\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::KubectlPods);
}

#[test]
fn summarizes_kubectl_logs_with_namespace_flag_before_target() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "kubectl".into(),
            "logs".into(),
            "-n".into(),
            "default".into(),
            "api-0".into(),
        ]),
        stdout: "booting\nERROR database unavailable\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "logs for api-0: 1 errors, 0 warnings");
}

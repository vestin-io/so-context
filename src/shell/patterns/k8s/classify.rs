use super::super::super::types::ShellPattern;
use super::super::argv::positional_args;

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    if program != "kubectl" {
        return None;
    }

    let positionals = positional_args(
        args,
        &[
            "-n",
            "--namespace",
            "-c",
            "--container",
            "--context",
            "--selector",
            "-l",
            "--field-selector",
            "--tail",
            "--since",
            "-o",
            "--output",
            "-f",
            "--filename",
            "--kubeconfig",
        ],
    );
    match positionals.first().copied() {
        Some("get")
            if positionals
                .iter()
                .any(|arg| matches!(*arg, "po" | "pod" | "pods")) =>
        {
            Some(ShellPattern::KubectlPods)
        }
        Some("get")
            if positionals
                .iter()
                .any(|arg| matches!(*arg, "svc" | "service" | "services")) =>
        {
            Some(ShellPattern::KubectlServices)
        }
        Some("logs") => Some(ShellPattern::KubectlLogs),
        Some("describe") => Some(ShellPattern::KubectlDescribe),
        Some("apply") => Some(ShellPattern::KubectlApply),
        _ => None,
    }
}

pub(super) fn logs_target(args: &[String]) -> Option<String> {
    kubectl_positionals(args).find(|arg| arg != "logs")
}

fn kubectl_positionals(args: &[String]) -> impl Iterator<Item = String> + '_ {
    let mut values = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with("--namespace=") || arg.starts_with("--container=") {
            continue;
        }
        if matches!(
            arg.as_str(),
            "-n" | "--namespace"
                | "-c"
                | "--container"
                | "--context"
                | "--selector"
                | "-l"
                | "--field-selector"
                | "--tail"
                | "--since"
        ) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        values.push(arg.clone());
    }
    values.into_iter()
}

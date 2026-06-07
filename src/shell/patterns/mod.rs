mod argv;
mod build;
mod docker;
mod generic;
mod gh;
mod git;
mod k8s;
mod node;
mod rust;
mod text;

use super::types::{CompressionSummary, ShellPattern, ShellResult};
use crate::shell::parse_simple_shell_command;

struct FamilyHandler {
    classify: fn(&str, &[String]) -> Option<ShellPattern>,
    summarize: fn(&ShellResult, ShellPattern) -> Option<CompressionSummary>,
}

#[derive(Debug, Clone, Copy)]
struct ClassifiedPattern {
    handler_index: usize,
    pattern: ShellPattern,
}

pub(super) fn compress(result: &ShellResult) -> CompressionSummary {
    let Some(classified) = classify(result) else {
        return generic::summarize_unknown(result);
    };

    let handler = &family_handlers()[classified.handler_index];
    if let Some(summary) = (handler.summarize)(result, classified.pattern) {
        return summary;
    }

    generic::summarize_fallback(result, classified.pattern)
}

pub(super) fn classify_only(result: &ShellResult) -> ShellPattern {
    classify(result)
        .map(|classified| classified.pattern)
        .unwrap_or(ShellPattern::Unknown)
}

fn classify(result: &ShellResult) -> Option<ClassifiedPattern> {
    let (program, args) = classification_target(result);

    for (handler_index, handler) in family_handlers().iter().enumerate() {
        if let Some(pattern) = (handler.classify)(&program, &args) {
            return Some(ClassifiedPattern {
                handler_index,
                pattern,
            });
        }
    }

    None
}

fn classification_target(result: &ShellResult) -> (String, Vec<String>) {
    let program = result.invocation.program().to_string();
    let args = result.invocation.args().to_vec();
    unwrap_shell_c_command(&program, &args).unwrap_or((program, args))
}

fn unwrap_shell_c_command(program: &str, args: &[String]) -> Option<(String, Vec<String>)> {
    if !matches!(program, "sh" | "bash" | "zsh") {
        return None;
    }

    let command_index = args
        .iter()
        .position(|arg| arg.starts_with('-') && arg.contains('c'))?;
    let command = args.get(command_index + 1)?;
    let argv = parse_simple_shell_command(command)?;
    let inner = strip_env_prefix(&argv);
    let inner_program = inner.first()?.clone();
    Some((inner_program, inner[1..].to_vec()))
}

fn strip_env_prefix(argv: &[String]) -> &[String] {
    let mut start = 0usize;
    if argv.first().is_some_and(|arg| arg == "env") {
        start += 1;
    }
    while argv.get(start).is_some_and(|arg| is_env_assignment(arg)) {
        start += 1;
    }
    &argv[start..]
}

fn is_env_assignment(arg: &str) -> bool {
    let Some((name, _value)) = arg.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn family_handlers() -> &'static [FamilyHandler] {
    &[
        FamilyHandler {
            classify: git::classify,
            summarize: git::summarize_pattern,
        },
        FamilyHandler {
            classify: docker::classify,
            summarize: docker::summarize_pattern,
        },
        FamilyHandler {
            classify: node::classify,
            summarize: node::summarize_pattern,
        },
        FamilyHandler {
            classify: rust::classify,
            summarize: rust::summarize_pattern,
        },
        FamilyHandler {
            classify: gh::classify,
            summarize: gh::summarize_pattern,
        },
        FamilyHandler {
            classify: k8s::classify,
            summarize: k8s::summarize_pattern,
        },
        FamilyHandler {
            classify: build::classify,
            summarize: build::summarize_pattern,
        },
        FamilyHandler {
            classify: generic::classify,
            summarize: generic::summarize_pattern,
        },
    ]
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

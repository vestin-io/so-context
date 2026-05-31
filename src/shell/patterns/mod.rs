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
    let program = result.invocation.program();
    let args = result.invocation.args();

    for (handler_index, handler) in family_handlers().iter().enumerate() {
        if let Some(pattern) = (handler.classify)(program, args) {
            return Some(ClassifiedPattern {
                handler_index,
                pattern,
            });
        }
    }

    None
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

use crate::shell::types::{CompressionSummary, ShellPattern, ShellResult};

const FAILURE_LIMIT: usize = 5;

#[derive(Debug, Clone)]
pub(super) struct FailureRecord {
    pub(super) name: String,
    pub(super) message_lines: Vec<String>,
}

pub(super) struct TestPresentation {
    pub(super) summary: String,
    pub(super) failures: Vec<FailureRecord>,
    pub(super) duration_ms: Option<u64>,
}

pub(super) fn render_test_presentation(
    pattern: ShellPattern,
    presentation: TestPresentation,
    result: &ShellResult,
) -> CompressionSummary {
    let mut details = Vec::new();

    for (index, failure) in presentation.failures.iter().take(FAILURE_LIMIT).enumerate() {
        details.push(format!(
            "{}. {}",
            index + 1,
            super::super::text::truncate_text(&failure.name, 140)
        ));
        for line in &failure.message_lines {
            details.push(format!("   {line}"));
        }
    }

    if presentation.failures.len() > FAILURE_LIMIT {
        details.push(format!(
            "... +{} more failures",
            presentation.failures.len() - FAILURE_LIMIT
        ));
    }

    if let Some(duration_ms) = presentation.duration_ms {
        if !details.is_empty() {
            details.push(String::new());
        }
        details.push(format!("Time: {duration_ms}ms"));
    }

    CompressionSummary::plain(pattern, presentation.summary, details, Vec::new(), result)
}

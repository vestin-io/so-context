use crate::shell::types::ShellResult;

pub fn preview(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .map(|line| line.trim().to_string())
        .collect()
}

pub fn sample_lines(lines: Vec<String>, limit: usize) -> Vec<String> {
    lines.into_iter().take(limit).collect()
}

pub fn append_omitted_line(details: &mut Vec<String>, total: usize, shown: usize, label: &str) {
    if total > shown {
        details.push(format!("+ {} more {label}", total - shown));
    }
}

pub fn non_empty_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

pub fn preferred_output(result: &ShellResult) -> String {
    match (
        result.stdout.trim().is_empty(),
        result.stderr.trim().is_empty(),
    ) {
        (false, true) => result.stdout.clone(),
        (true, false) => result.stderr.clone(),
        (false, false) => format!("{}\n{}", result.stdout, result.stderr),
        (true, true) => String::new(),
    }
}

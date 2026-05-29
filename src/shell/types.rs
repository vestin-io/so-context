use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Compressed,
}

#[derive(Debug, Clone)]
pub struct ShellInvocation {
    pub argv: Vec<String>,
}

impl ShellInvocation {
    pub fn new(argv: Vec<String>) -> Self {
        Self { argv }
    }

    pub fn program(&self) -> &str {
        &self.argv[0]
    }

    pub fn args(&self) -> &[String] {
        &self.argv[1..]
    }

    pub fn command_line(&self) -> String {
        self.argv.join(" ")
    }
}

#[derive(Debug, Clone)]
pub struct ShellResult {
    pub invocation: ShellInvocation,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub rendered: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPattern {
    GitStatus,
    GitDiff,
    GitLog,
    GitBranch,
    GitRemote,
    GitShow,
    GitFetch,
    GitPull,
    GitPush,
    GitCheckout,
    GitSwitch,
    GitCommit,
    DockerPs,
    DockerImages,
    DockerCompose,
    DockerLogs,
    DockerBuild,
    DockerInspect,
    DockerPull,
    Ls,
    Find,
    Rg,
    Grep,
    Curl,
    Wget,
    Env,
    Cat,
    Head,
    Tail,
    Unknown,
}

impl ShellPattern {
    pub fn label(self) -> &'static str {
        match self {
            Self::GitStatus => "git.status",
            Self::GitDiff => "git.diff",
            Self::GitLog => "git.log",
            Self::GitBranch => "git.branch",
            Self::GitRemote => "git.remote",
            Self::GitShow => "git.show",
            Self::GitFetch => "git.fetch",
            Self::GitPull => "git.pull",
            Self::GitPush => "git.push",
            Self::GitCheckout => "git.checkout",
            Self::GitSwitch => "git.switch",
            Self::GitCommit => "git.commit",
            Self::DockerPs => "docker.ps",
            Self::DockerImages => "docker.images",
            Self::DockerCompose => "docker.compose",
            Self::DockerLogs => "docker.logs",
            Self::DockerBuild => "docker.build",
            Self::DockerInspect => "docker.inspect",
            Self::DockerPull => "docker.pull",
            Self::Ls => "generic.ls",
            Self::Find => "generic.find",
            Self::Rg => "generic.rg",
            Self::Grep => "generic.grep",
            Self::Curl => "generic.curl",
            Self::Wget => "generic.wget",
            Self::Env => "generic.env",
            Self::Cat => "generic.cat",
            Self::Head => "generic.head",
            Self::Tail => "generic.tail",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionSummary {
    pub pattern: ShellPattern,
    pub summary: String,
    pub details: Vec<String>,
    pub stderr_preview: Vec<String>,
    pub exit_code: i32,
    pub command_line: String,
}

impl CompressionSummary {
    pub fn render(&self, mode: RenderMode, debug: bool) -> String {
        let mut output = String::new();
        if debug {
            let _ = writeln!(output, "pattern: {}", self.pattern.label());
            let _ = writeln!(output, "exit_code: {}", self.exit_code);
            let _ = writeln!(output, "command: {}", self.command_line);
        }

        let _ = writeln!(output, "{}", self.summary);

        if matches!(mode, RenderMode::Compressed) && !self.details.is_empty() {
            for line in &self.details {
                let _ = writeln!(output, "- {line}");
            }
        }

        if !self.stderr_preview.is_empty() {
            if !output.ends_with('\n') {
                let _ = writeln!(output);
            }
            let _ = writeln!(output, "stderr:");
            for line in &self.stderr_preview {
                let _ = writeln!(output, "- {line}");
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::{CompressionSummary, RenderMode, ShellPattern};

    fn sample_summary() -> CompressionSummary {
        CompressionSummary {
            pattern: ShellPattern::GitDiff,
            summary: "files=2; hunks=3; additions=10; deletions=4".into(),
            details: vec!["src/main.rs".into(), "README.md".into()],
            stderr_preview: vec!["warning: demo".into()],
            exit_code: 0,
            command_line: "git diff".into(),
        }
    }

    #[test]
    fn default_render_hides_debug_metadata() {
        let rendered = sample_summary().render(RenderMode::Compressed, false);

        assert!(rendered.starts_with("files=2; hunks=3; additions=10; deletions=4\n"));
        assert!(rendered.contains("- src/main.rs\n"));
        assert!(rendered.contains("stderr:\n- warning: demo\n"));
        assert!(!rendered.contains("pattern:"));
        assert!(!rendered.contains("exit_code:"));
        assert!(!rendered.contains("command:"));
    }

    #[test]
    fn debug_render_includes_metadata() {
        let rendered = sample_summary().render(RenderMode::Compressed, true);

        assert!(rendered.contains("pattern: git.diff\n"));
        assert!(rendered.contains("exit_code: 0\n"));
        assert!(rendered.contains("command: git diff\n"));
        assert!(rendered.contains("files=2; hunks=3; additions=10; deletions=4\n"));
    }
}

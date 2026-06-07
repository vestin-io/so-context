use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ShellInvocation {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
}

impl ShellInvocation {
    pub fn new(argv: Vec<String>) -> Self {
        Self { argv, cwd: None }
    }

    pub fn with_cwd(argv: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            argv,
            cwd: Some(cwd),
        }
    }

    pub fn program(&self) -> &str {
        &self.argv[0]
    }

    pub fn args(&self) -> &[String] {
        &self.argv[1..]
    }

    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    pub fn command_line(&self) -> String {
        Self::render_argv(&self.argv)
    }

    pub(crate) fn render_argv(argv: &[String]) -> String {
        argv.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone)]
pub struct ShellResult {
    pub invocation: ShellInvocation,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl ShellResult {
    pub fn render_full(&self) -> String {
        match (self.stdout.is_empty(), self.stderr.is_empty()) {
            (false, true) => self.stdout.clone(),
            (true, false) => with_trailing_newline(&self.stderr),
            _ => self.render_full_body(),
        }
    }

    fn render_full_body(&self) -> String {
        let mut output = String::with_capacity(self.stdout.len() + self.stderr.len() + 1);

        if !self.stdout.is_empty() {
            output.push_str(&self.stdout);
            if !self.stdout.ends_with('\n') && !self.stderr.is_empty() {
                output.push('\n');
            }
        }
        if !self.stderr.is_empty() {
            output.push_str(&self.stderr);
            if !self.stderr.ends_with('\n') {
                output.push('\n');
            }
        }
        output
    }
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub run_id: String,
    pub invocation: ShellInvocation,
    pub pattern: ShellPattern,
    pub rendered: String,
    pub full_output: String,
    pub exit_code: i32,
    pub output_mode: ShellOutputMode,
    pub requested_full: bool,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellOutputMode {
    Compressed,
    RawFallback,
    Full,
}

impl ShellOutputMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Compressed => "compressed",
            Self::RawFallback => "raw_fallback",
            Self::Full => "full",
        }
    }
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
    GitAdd,
    GitClone,
    GitMerge,
    GitTag,
    GitReset,
    GitStash,
    DockerPs,
    DockerImages,
    DockerCompose,
    DockerLogs,
    DockerBuild,
    DockerInspect,
    DockerPull,
    NodeNpm,
    NodePnpm,
    NodeYarn,
    NodeBun,
    NodeNpx,
    CargoBuild,
    CargoTest,
    CargoClippy,
    CargoCheck,
    CargoInstall,
    CargoNextest,
    GhPr,
    GhIssue,
    GhRun,
    KubectlPods,
    KubectlServices,
    KubectlLogs,
    KubectlDescribe,
    KubectlApply,
    Tsc,
    NextBuild,
    ViteBuild,
    Make,
    Gradle,
    Maven,
    DotnetBuild,
    DotnetTest,
    DotnetRestore,
    DotnetFormat,
    Cmake,
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
    TextExcerpt,
    RgFiles,
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
            Self::GitAdd => "git.add",
            Self::GitClone => "git.clone",
            Self::GitMerge => "git.merge",
            Self::GitTag => "git.tag",
            Self::GitReset => "git.reset",
            Self::GitStash => "git.stash",
            Self::DockerPs => "docker.ps",
            Self::DockerImages => "docker.images",
            Self::DockerCompose => "docker.compose",
            Self::DockerLogs => "docker.logs",
            Self::DockerBuild => "docker.build",
            Self::DockerInspect => "docker.inspect",
            Self::DockerPull => "docker.pull",
            Self::NodeNpm => "node.npm",
            Self::NodePnpm => "node.pnpm",
            Self::NodeYarn => "node.yarn",
            Self::NodeBun => "node.bun",
            Self::NodeNpx => "node.npx",
            Self::CargoBuild => "rust.cargo-build",
            Self::CargoTest => "rust.cargo-test",
            Self::CargoClippy => "rust.cargo-clippy",
            Self::CargoCheck => "rust.cargo-check",
            Self::CargoInstall => "rust.cargo-install",
            Self::CargoNextest => "rust.cargo-nextest",
            Self::GhPr => "gh.pr",
            Self::GhIssue => "gh.issue",
            Self::GhRun => "gh.run",
            Self::KubectlPods => "k8s.kubectl-pods",
            Self::KubectlServices => "k8s.kubectl-services",
            Self::KubectlLogs => "k8s.kubectl-logs",
            Self::KubectlDescribe => "k8s.kubectl-describe",
            Self::KubectlApply => "k8s.kubectl-apply",
            Self::Tsc => "build.tsc",
            Self::NextBuild => "build.next",
            Self::ViteBuild => "build.vite",
            Self::Make => "build.make",
            Self::Gradle => "build.gradle",
            Self::Maven => "build.maven",
            Self::DotnetBuild => "build.dotnet-build",
            Self::DotnetTest => "build.dotnet-test",
            Self::DotnetRestore => "build.dotnet-restore",
            Self::DotnetFormat => "build.dotnet-format",
            Self::Cmake => "build.cmake",
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
            Self::TextExcerpt => "generic.text-excerpt",
            Self::RgFiles => "generic.rg-files",
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
    pub render_style: CompressionRenderStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionRenderStyle {
    Bulleted,
    Plain,
}

impl CompressionSummary {
    pub fn new(
        pattern: ShellPattern,
        summary: impl Into<String>,
        details: Vec<String>,
        stderr_preview: Vec<String>,
        _result: &ShellResult,
    ) -> Self {
        Self {
            pattern,
            summary: summary.into(),
            details,
            stderr_preview,
            render_style: CompressionRenderStyle::Bulleted,
        }
    }

    pub fn plain(
        pattern: ShellPattern,
        summary: impl Into<String>,
        details: Vec<String>,
        stderr_preview: Vec<String>,
        _result: &ShellResult,
    ) -> Self {
        Self {
            pattern,
            summary: summary.into(),
            details,
            stderr_preview,
            render_style: CompressionRenderStyle::Plain,
        }
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        if !self.summary.is_empty() {
            output.push_str(&self.summary);
            output.push('\n');
        }

        if !self.details.is_empty() {
            for line in &self.details {
                if self.render_style == CompressionRenderStyle::Bulleted {
                    output.push_str("- ");
                }
                output.push_str(line);
                output.push('\n');
            }
        }

        if !self.stderr_preview.is_empty() {
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("stderr:\n");
            for line in &self.stderr_preview {
                output.push_str("- ");
                output.push_str(line);
                output.push('\n');
            }
        }

        output
    }
}

fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }

    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn with_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;

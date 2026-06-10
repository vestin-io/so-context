use std::env;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use super::exec;
use super::output_spool::{self, SpoolOwner};
use super::patterns;
use super::types::{RunOutput, ShellInvocation, ShellOutputMode, ShellResult};

const RUN_ID_ENV: &str = "SO_CONTEXT_SHELL_RUN_ID";

static RUN_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct ShellRunOptions {
    pub full: bool,
    pub spool_owner: Option<SpoolOwner>,
}

impl ShellRunOptions {
    pub fn new(full: bool) -> Self {
        Self {
            full,
            spool_owner: None,
        }
    }

    pub fn with_spool_owner(mut self, owner: SpoolOwner) -> Self {
        self.spool_owner = Some(owner);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ShellRunner {
    options: ShellRunOptions,
}

impl ShellRunner {
    pub fn new(options: ShellRunOptions) -> Self {
        Self { options }
    }

    pub fn run_in_dir(&self, argv: &[String], cwd: Option<PathBuf>) -> Result<RunOutput> {
        if argv.is_empty() {
            bail!("shell command requires at least one argument");
        }

        let invocation = match cwd {
            Some(cwd) => ShellInvocation::with_cwd(argv.to_vec(), cwd),
            None => ShellInvocation::new(argv.to_vec()),
        };
        self.run_invocation(invocation)
    }

    fn run_invocation(&self, invocation: ShellInvocation) -> Result<RunOutput> {
        let run_id = resolve_run_id();
        let result = if self.options.full {
            exec::execute_full(invocation)?
        } else {
            exec::execute(invocation)?
        };
        if self.options.full {
            return self.render_full_output(&run_id, result);
        }

        let full_output = result.render_full();
        let compressed = patterns::compress(&result);
        let compressed_rendered = compressed.render();
        let (rendered, output_mode) =
            self.select_rendered_output(&full_output, compressed_rendered);

        let output = RunOutput {
            run_id: run_id.to_string(),
            invocation: result.invocation.clone(),
            pattern: compressed.pattern,
            rendered,
            full_output,
            exit_code: result.exit_code,
            output_mode,
            requested_full: false,
            capture: result.capture,
        };
        output_spool::store_run_output(&output, self.options.spool_owner.as_ref());
        Ok(output)
    }

    fn render_full_output(&self, run_id: &str, result: ShellResult) -> Result<RunOutput> {
        let pattern = patterns::classify_only(&result);
        let full_output = result.render_full();

        let output = RunOutput {
            run_id: run_id.to_string(),
            invocation: result.invocation.clone(),
            pattern,
            rendered: None,
            full_output,
            exit_code: result.exit_code,
            output_mode: ShellOutputMode::Full,
            requested_full: true,
            capture: result.capture,
        };
        output_spool::store_run_output(&output, self.options.spool_owner.as_ref());
        Ok(output)
    }

    fn select_rendered_output(
        &self,
        full_output: &str,
        compressed_rendered: String,
    ) -> (Option<String>, ShellOutputMode) {
        if compressed_rendered.len() >= full_output.len() {
            return (None, ShellOutputMode::RawFallback);
        }

        (Some(compressed_rendered), ShellOutputMode::Compressed)
    }
}

pub fn resolve_run_id() -> String {
    if let Ok(run_id) = env::var(RUN_ID_ENV)
        && !run_id.trim().is_empty()
    {
        return run_id;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let sequence = RUN_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("shell-{}-{}-{}", process::id(), nanos, sequence)
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;

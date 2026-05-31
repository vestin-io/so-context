use std::env;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use super::exec;
use super::patterns;
use super::record;
use super::types::{RunOutput, ShellInvocation, ShellOutputMode, ShellPattern, ShellResult};

const RUN_ID_ENV: &str = "SO_CONTEXT_SHELL_RUN_ID";

static RUN_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy)]
pub struct ShellRunOptions {
    pub full: bool,
}

impl ShellRunOptions {
    pub fn new(full: bool) -> Self {
        Self { full }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellRunner {
    options: ShellRunOptions,
}

impl ShellRunner {
    pub fn new(options: ShellRunOptions) -> Self {
        Self { options }
    }

    pub fn run(&self, argv: &[String]) -> Result<RunOutput> {
        if argv.is_empty() {
            bail!("shell command requires at least one argument");
        }

        let invocation = ShellInvocation::new(argv.to_vec());
        let run_id = resolve_run_id();
        let result = exec::execute(invocation)?;
        if self.options.full {
            return self.render_full_output(&run_id, result);
        }

        let compressed = patterns::compress(&result);
        let compressed_rendered = compressed.render();
        let (rendered, output_mode) = self.select_rendered_output(&result, compressed_rendered);
        self.persist_result(&run_id, &result, compressed.pattern, &rendered, output_mode);

        Ok(RunOutput {
            rendered,
            exit_code: result.exit_code,
        })
    }

    fn render_full_output(&self, run_id: &str, result: ShellResult) -> Result<RunOutput> {
        let pattern = patterns::classify_only(&result);
        let rendered = result.render_full();
        self.persist_result(run_id, &result, pattern, &rendered, ShellOutputMode::Full);

        Ok(RunOutput {
            rendered,
            exit_code: result.exit_code,
        })
    }

    fn select_rendered_output(
        &self,
        result: &ShellResult,
        compressed_rendered: String,
    ) -> (String, ShellOutputMode) {
        if compressed_rendered.len() >= result.render_full_len() {
            return (result.render_full(), ShellOutputMode::RawFallback);
        }

        (compressed_rendered, ShellOutputMode::Compressed)
    }

    fn persist_result(
        &self,
        run_id: &str,
        result: &ShellResult,
        pattern: ShellPattern,
        rendered: &str,
        output_mode: ShellOutputMode,
    ) {
        let _ = record::persist(run_id, result, pattern, rendered, output_mode);
    }
}

fn resolve_run_id() -> String {
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

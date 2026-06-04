use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::types::{ShellInvocation, ShellResult};

/// Per-stream capture limit aligned with RTK `RAW_CAP` (10 MiB).
const MAX_STDOUT_BYTES: usize = 10_485_760;
const MAX_STDERR_BYTES: usize = 10_485_760;
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 30_000;
const COMMAND_TIMEOUT_ENV: &str = "SO_CONTEXT_SHELL_TIMEOUT_MS";

#[derive(Debug, Clone, Copy)]
struct ExecLimits {
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    timeout: Duration,
}

impl Default for ExecLimits {
    fn default() -> Self {
        Self {
            max_stdout_bytes: MAX_STDOUT_BYTES,
            max_stderr_bytes: MAX_STDERR_BYTES,
            timeout: command_timeout(),
        }
    }
}

#[derive(Debug)]
struct StreamCapture {
    text: String,
    truncated: bool,
}

pub(super) fn execute(invocation: ShellInvocation) -> Result<ShellResult> {
    execute_with_limits(invocation, ExecLimits::default())
}

fn execute_with_limits(invocation: ShellInvocation, limits: ExecLimits) -> Result<ShellResult> {
    let mut command = Command::new(invocation.execution_program());
    command
        .args(invocation.execution_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = invocation.cwd() {
        command.current_dir(cwd);
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute command: {}", invocation.command_line()))?;

    let stdout = child
        .stdout
        .take()
        .context("child stdout pipe was not captured")?;
    let stderr = child
        .stderr
        .take()
        .context("child stderr pipe was not captured")?;
    let stdout_thread = thread::spawn(move || capture_stream(stdout, limits.max_stdout_bytes));
    let stderr_thread = thread::spawn(move || capture_stream(stderr, limits.max_stderr_bytes));

    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child
            .try_wait()
            .context("failed to wait for shell command status")?
        {
            break (status, false);
        }

        if started.elapsed() >= limits.timeout {
            let _ = child.kill();
            let status = child
                .wait()
                .context("failed to collect timed out shell command status")?;
            break (status, true);
        }

        thread::sleep(Duration::from_millis(10));
    };

    let stdout = stdout_thread
        .join()
        .expect("stdout reader thread panicked")
        .context("failed to capture stdout")?;
    let stderr = stderr_thread
        .join()
        .expect("stderr reader thread panicked")
        .context("failed to capture stderr")?;

    let notes = execution_notes(&stdout, &stderr, timed_out, limits);

    Ok(ShellResult {
        invocation,
        stdout: stdout.text,
        stderr: prepend_notes(notes, stderr.text),
        exit_code: if timed_out {
            124
        } else {
            status.code().unwrap_or(1)
        },
    })
}

fn capture_stream(mut reader: impl Read, max_bytes: usize) -> Result<StreamCapture> {
    let mut buf = [0_u8; 8192];
    let mut captured = Vec::new();
    let mut total = 0usize;

    loop {
        let read = reader
            .read(&mut buf)
            .context("failed to read shell output")?;
        if read == 0 {
            break;
        }

        total += read;
        let remaining = max_bytes.saturating_sub(captured.len());
        if remaining > 0 {
            let kept = read.min(remaining);
            captured.extend_from_slice(&buf[..kept]);
        }
    }

    Ok(StreamCapture {
        text: String::from_utf8_lossy(&captured).into_owned(),
        truncated: total > captured.len(),
    })
}

fn execution_notes(
    stdout: &StreamCapture,
    stderr: &StreamCapture,
    timed_out: bool,
    limits: ExecLimits,
) -> Vec<String> {
    let mut notes = Vec::new();

    if stdout.truncated {
        notes.push(format!(
            "[shell] stdout truncated after {} bytes",
            limits.max_stdout_bytes
        ));
    }
    if stderr.truncated {
        notes.push(format!(
            "[shell] stderr truncated after {} bytes",
            limits.max_stderr_bytes
        ));
    }
    if timed_out {
        notes.push(format!(
            "[shell] command timed out after {}ms",
            limits.timeout.as_millis()
        ));
    }

    notes
}

fn command_timeout() -> Duration {
    let timeout_ms = std::env::var(COMMAND_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS);
    Duration::from_millis(timeout_ms)
}

fn prepend_notes(notes: Vec<String>, stderr: String) -> String {
    if notes.is_empty() {
        return stderr;
    }

    let mut rendered = notes.join("\n");
    if !stderr.is_empty() {
        rendered.push('\n');
        rendered.push_str(&stderr);
    } else {
        rendered.push('\n');
    }
    rendered
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;

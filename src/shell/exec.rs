use std::process::Command;

use anyhow::{Context, Result};

use super::types::{ShellInvocation, ShellResult};

pub fn execute(invocation: ShellInvocation) -> Result<ShellResult> {
    let output = Command::new(invocation.program())
        .args(invocation.args())
        .output()
        .with_context(|| format!("failed to execute command: {}", invocation.command_line()))?;

    Ok(ShellResult {
        invocation,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(1),
    })
}

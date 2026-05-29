mod exec;
mod patterns;
mod record;
mod types;

use anyhow::{Result, bail};

pub use types::RunOutput;
use types::{RenderMode, ShellInvocation, ShellResult};

pub fn run(argv: &[String], full: bool, record_raw: bool, debug_shell: bool) -> Result<RunOutput> {
    if argv.is_empty() {
        bail!("shell command requires at least one argument");
    }

    let invocation = ShellInvocation::new(argv.to_vec());
    let result = execute(invocation)?;
    let compressed = patterns::compress(&result);
    let compressed_rendered = compressed.render(RenderMode::Compressed, false);

    if let Err(err) = record::persist(&result, &compressed, &compressed_rendered, record_raw) {
        if debug_shell {
            eprintln!("[shell] failed to record metrics: {err}");
        }
    }

    Ok(RunOutput {
        rendered: if full {
            render_full(&result, debug_shell)
        } else {
            compressed.render(RenderMode::Compressed, debug_shell)
        },
        exit_code: result.exit_code,
    })
}

fn execute(invocation: ShellInvocation) -> Result<ShellResult> {
    exec::execute(invocation)
}

fn render_full(result: &ShellResult, debug_shell: bool) -> String {
    let mut output = String::new();

    if debug_shell {
        output.push_str(&format!("pattern: full\n"));
        output.push_str(&format!("exit_code: {}\n", result.exit_code));
        output.push_str(&format!("command: {}\n", result.invocation.command_line()));
    }

    if !result.stdout.is_empty() {
        output.push_str(&result.stdout);
        if !result.stdout.ends_with('\n') && !result.stderr.is_empty() {
            output.push('\n');
        }
    }

    if !result.stderr.is_empty() {
        output.push_str(&result.stderr);
        if !result.stderr.ends_with('\n') {
            output.push('\n');
        }
    }

    output
}

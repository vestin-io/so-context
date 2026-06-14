use std::process;

use anyhow::{Error, Result};

use crate::core_event_spool::{resolve_reliable_project_root, spool_global_event_record};
use crate::core_events::{EventRecord, Timer};

use super::{
    ShellEventContext, ShellRunOptions, ShellRunner, build_shell_error_event, build_shell_event,
    logical_argv_for_shell_command, resolve_run_id,
};

pub fn run_shell_cli_command(full: bool, command: Option<String>, argv: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir().ok();
    let event_project = resolve_reliable_project_root(cwd.as_deref()).or_else(|| cwd.clone());
    let timer = Timer::start();
    let runner = ShellRunner::new(ShellRunOptions::new(full));
    let output = match command.as_deref() {
        Some(command) => runner.run_command_string(command, cwd.clone()),
        None => runner.run(&argv),
    };
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            let context = ShellEventContext::cli_shell(resolve_run_id(), event_project.clone());
            let error_argv = command
                .as_deref()
                .map(logical_argv_for_shell_command)
                .unwrap_or(argv);
            let event = build_shell_error_event(
                context,
                &error_argv,
                cwd.as_deref(),
                full,
                timer.elapsed_ms(),
                &error.to_string(),
            );
            if let Err(persist_error) = persist_shell_cli_event(&event) {
                eprintln!("so-context shell: failed to record error event: {persist_error}");
            }
            return Err(error);
        }
    };

    let displayed_output = output.displayed_output().to_string();
    let exit_code = output.exit_code;
    let context = ShellEventContext::cli_shell(output.run_id.clone(), event_project);
    let event = build_shell_event(context, &output, &displayed_output, timer.elapsed_ms());
    if let Err(persist_error) = persist_shell_cli_event(&event) {
        eprintln!("so-context shell: failed to record event: {persist_error}");
    }
    print!("{displayed_output}");
    if exit_code != 0 {
        process::exit(exit_code);
    }

    Ok(())
}

fn persist_shell_cli_event(event: &EventRecord) -> Result<()> {
    spool_global_event_record(event)
        .map(|_| ())
        .map_err(Error::msg)
}

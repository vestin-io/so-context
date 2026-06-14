mod cli;
mod command_string;
mod exec;
mod output_spool;
mod patterns;
mod policy;
mod redaction;
mod redirect;
mod runner;
mod telemetry;
pub(crate) mod types;

pub use cli::run_shell_cli_command;
pub use command_string::{
    logical_argv_for_shell_command, parse_simple_shell_command, rewrite_env_prefix, shell_flag,
};
pub use output_spool::{SpoolOwner, SpooledShellOutput, spooled_output};
pub use policy::{NATIVE_SHELL_TOOL_NAMES, should_prefer_so_shell};
pub use redirect::{ShellRedirectContext, rewrite_native_shell_tool_input};
pub use runner::{ShellRunOptions, ShellRunner, resolve_run_id};
pub use telemetry::{ShellEventContext, build_shell_error_event, build_shell_event};
pub use types::{RunOutput, ShellOutputMode};

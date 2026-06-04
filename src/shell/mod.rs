mod command_string;
mod exec;
mod output_spool;
mod patterns;
mod policy;
mod redaction;
mod runner;
mod telemetry;
mod types;

pub use command_string::{parse_simple_shell_command, rewrite_env_prefix};
pub use output_spool::{SpoolOwner, get_spooled_output};
pub use policy::{NATIVE_SHELL_TOOL_NAMES, native_shell_policy_json, should_prefer_so_shell};
pub use runner::{ShellRunOptions, ShellRunner};
pub use telemetry::{ShellEventContext, build_shell_error_event, build_shell_event};
pub use types::ShellOutputMode;

//! `so-context hook` — PreToolUse/PostCompact hook handlers for agent CLIs.

mod post_compact;
mod pre_tool;

pub use post_compact::run_post_compact_hook_for_host;
pub use pre_tool::run_pre_tool_use_hook_for_host;

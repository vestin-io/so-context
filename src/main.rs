#[path = "core/events.rs"]
pub mod core_events;
#[path = "core/graph/mod.rs"]
mod core_graph;
#[path = "core/read.rs"]
mod core_read;
#[path = "core/tokens.rs"]
pub mod core_tokens;
mod daemon;
#[path = "core/file_visit_cache.rs"]
pub mod file_visit_cache;
mod hook;
mod mcp;

use libc;
mod setup;
mod shell;
mod socket;

use anyhow::Result;
use clap::{Parser, Subcommand};
use daemon::Daemon;

#[derive(Parser, Debug)]
#[command(
    name = "so-context",
    version,
    about = "CLI + MCP daemon for code graph indexing"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum HookCommands {
    /// PreToolUse hook handler for agent CLIs (Claude Code, Codex).
    ///
    /// Reads the hook JSON from stdin. For so-context MCP tools it injects
    /// `_so_session_id`. For selected native read and short, one-shot native
    /// shell calls it blocks the tool call and tells the agent to retry with
    /// `mcp__so-context__so_read` or `mcp__so-context__so_shell`.
    /// Exit 0 with no output for everything else (agent continues normally).
    ///
    /// Register as a PreToolUse hook with matchers `"mcp__so-context__.*"`
    /// plus the read/shell-tool aliases this agent exposes (for example
    /// `"Read"`, `"View"`, `"Bash"`, or `"runTerminalCommand"`).
    PreTool,
    /// PostCompact hook handler for Claude Code.
    ///
    /// Reads the PostCompact hook JSON from stdin and tells the running daemon
    /// to reset file-visit cache entries for the compacted session, so the
    /// agent receives full file content again after compaction.
    ///
    /// Register as a PostCompact hook (no matcher needed).
    PostCompact,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the background daemon: binds a Unix socket and serves the MCP HTTP server.
    /// Start this once; it stays alive across multiple agent sessions.
    Daemon,
    /// Start the MCP stdio bridge for an agent session.
    /// Connects to the running daemon and forwards MCP messages over stdio.
    /// This is the command to register in Claude / OpenCode / Codex configs.
    Mcp,
    /// Hook handlers for agent CLIs (Claude Code, Codex).
    ///
    /// Use `hook pre-tool` or `hook post-compact` depending on the event.
    Hook {
        #[command(subcommand)]
        event: HookCommands,
    },
    /// Index a project folder into the local code graph SQLite database.
    /// With --watch, keeps running and re-indexes on file changes (foreground).
    Index {
        /// Project folder path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// Keep running and re-index on file changes (foreground).
        #[arg(long)]
        watch: bool,
    },
    /// Tell the running daemon to start watching a project directory.
    /// Intended for use in agent session-start hooks.
    Watch {
        /// Project directory to watch (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// MCP client name (e.g. "claude", "opencode", "codex").
        #[arg(long)]
        client: Option<String>,
        /// Session identifier for this consumer. Auto-generated if omitted.
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Tell the running daemon to stop watching a project directory.
    /// Intended for use in agent session-end hooks.
    Unwatch {
        /// Project directory to unwatch (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// Client name — must match the value passed to watch.
        #[arg(long)]
        client: Option<String>,
        /// Session ID — must match the value passed to watch.
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Install so-context as an MCP server in Claude, OpenCode, and Codex configs.
    Setup {
        /// Path to the so-context binary (default: current executable).
        #[arg(long)]
        binary: Option<String>,
    },
    /// Remove all so-context entries from Claude, OpenCode, and Codex configs.
    Uninstall {
        /// Path to the so-context binary used during setup (default: current executable).
        /// Used to identify hook commands to remove.
        #[arg(long)]
        binary: Option<String>,
    },
}

/// Parses CLI arguments and dispatches to the appropriate execution path.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Daemon => {
            // Ignore SIGHUP so the daemon survives terminal disconnects.
            #[cfg(unix)]
            unsafe {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
            }
            Daemon::new().run().await
        }
        Commands::Mcp => mcp::run_mcp_bridge().await,
        Commands::Hook { event } => match event {
            HookCommands::PreTool => hook::run_pre_tool_use_hook(),
            HookCommands::PostCompact => hook::run_post_compact_hook().await,
        },
        Commands::Index { path, watch } => {
            if watch {
                core_graph::watch_project(&path).map_err(anyhow::Error::msg)
            } else {
                let output = core_graph::index_project(&path).map_err(anyhow::Error::msg)?;
                println!("{output}");
                Ok(())
            }
        }
        Commands::Watch {
            path,
            client,
            session_id,
        } => mcp::send_ctrl_request("watch", &path, client.as_deref(), session_id.as_deref()).await,
        Commands::Unwatch {
            path,
            client,
            session_id,
        } => {
            mcp::send_ctrl_request("unwatch", &path, client.as_deref(), session_id.as_deref()).await
        }
        Commands::Setup { binary } => {
            let bin = resolve_binary(binary);
            setup::install_all(&bin)
        }
        Commands::Uninstall { binary } => {
            let bin = resolve_binary(binary);
            setup::uninstall_all(&bin)
        }
    }
}

fn resolve_binary(binary: Option<String>) -> String {
    binary.unwrap_or_else(|| {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "so-context".to_string())
    })
}

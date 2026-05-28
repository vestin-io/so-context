#[path = "core/read.rs"]
mod core_read;
#[path = "core/graph/mod.rs"]
mod core_graph;
#[path = "core/events.rs"]
pub mod core_events;
#[path = "core/tokens.rs"]
pub mod core_tokens;
mod daemon;
mod mcp;
mod setup;
mod socket;

use anyhow::Result;
use clap::{Parser, Subcommand};

use daemon::Daemon;

#[derive(Parser, Debug)]
#[command(name = "so-context", version, about = "CLI + MCP daemon for code graph indexing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    /// Index a project folder into the local code graph SQLite database.
    Index {
        /// Project folder path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
    },
    /// Index a project folder and keep watching for file changes (single project, foreground).
    Watch {
        /// Project folder path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
    },
    /// Tell the running daemon to start watching a project directory.
    /// Intended for use in agent session-start hooks.
    EnsureWatch {
        /// Project directory to watch (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// Name of the calling agent (e.g. "claude", "opencode", "codex").
        #[arg(long)]
        agent: Option<String>,
        /// Session identifier for this agent instance. Auto-generated if omitted.
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Tell the running daemon to stop watching a project directory.
    /// Intended for use in agent session-end hooks.
    Unwatch {
        /// Project directory to unwatch (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// Agent name — must match the value passed to ensure-watch.
        #[arg(long)]
        agent: Option<String>,
        /// Session ID — must match the value passed to ensure-watch.
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
        Commands::Daemon => Daemon::new().run().await,
        Commands::Mcp => mcp::run_mcp_bridge().await,
        Commands::Index { path } => {
            let output = core_graph::index_project(&path).map_err(anyhow::Error::msg)?;
            println!("{output}");
            Ok(())
        }
        Commands::Watch { path } => {
            core_graph::watch_project(&path).map_err(anyhow::Error::msg)
        }
        Commands::EnsureWatch { path, agent, session_id } => {
            mcp::send_ctrl_request("watch", &path, agent.as_deref(), session_id.as_deref()).await
        }
        Commands::Unwatch { path, agent, session_id } => {
            mcp::send_ctrl_request("unwatch", &path, agent.as_deref(), session_id.as_deref()).await
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

#[path = "core/events.rs"]
pub mod core_events;
#[path = "core/graph/mod.rs"]
mod core_graph;
#[path = "core/metrics.rs"]
mod core_metrics;
#[path = "core/read.rs"]
mod core_read;
#[path = "core/tokens.rs"]
pub mod core_tokens;
mod daemon;
#[path = "core/file_visit_cache.rs"]
pub mod file_visit_cache;
mod hook;
mod mcp;

mod setup;
mod shell;
mod socket;
mod update;
mod version;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use core_metrics::{MetricsWindow, render_metrics_text};
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

#[derive(Copy, Clone, Debug, ValueEnum)]
enum MetricsWindowArg {
    #[value(name = "24h")]
    H24,
    #[value(name = "7d")]
    D7,
    #[value(name = "all")]
    All,
}

impl From<MetricsWindowArg> for MetricsWindow {
    fn from(value: MetricsWindowArg) -> Self {
        match value {
            MetricsWindowArg::H24 => MetricsWindow::Last24Hours,
            MetricsWindowArg::D7 => MetricsWindow::Last7Days,
            MetricsWindowArg::All => MetricsWindow::All,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum MetricsFormatArg {
    Text,
    Json,
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
    /// Start the daemon in the background.
    Start,
    /// Stop the background daemon.
    Stop,
    /// Restart the background daemon.
    Restart,
    /// Update the current so-context binary to the latest release.
    Update,
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
    /// Show all projects currently watched by the running daemon.
    Status,
    /// Show usage metrics aggregated from so-context events.
    Metrics {
        /// Time window for event aggregation.
        #[arg(long, value_enum, default_value = "24h")]
        window: MetricsWindowArg,
        /// Restrict to one absolute project path.
        #[arg(long)]
        project: Option<String>,
        /// Number of top shell commands to show.
        #[arg(long, default_value_t = 5)]
        top: usize,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: MetricsFormatArg,
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
        Commands::Start => daemon::start_background(&resolve_binary(None)).await,
        Commands::Stop => daemon::stop_background().await,
        Commands::Restart => daemon::restart_background(&resolve_binary(None)).await,
        Commands::Update => update::update_current_binary().await,
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
        Commands::Status => {
            let text = mcp::send_ctrl_status_request().await?;
            println!("{text}");
            Ok(())
        }
        Commands::Metrics {
            window,
            project,
            top,
            format,
        } => {
            let summary =
                mcp::send_ctrl_metrics_request(window.into(), project.as_deref(), top).await?;
            match format {
                MetricsFormatArg::Text => println!("{}", render_metrics_text(&summary)),
                MetricsFormatArg::Json => {
                    println!("{}", serde_json::to_string_pretty(&summary)?)
                }
            }
            Ok(())
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

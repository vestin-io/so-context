#[path = "core/read.rs"]
mod core_read;
#[path = "core/graph/mod.rs"]
mod core_graph;
mod daemon;
mod mcp;
mod setup;

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
    /// Run the daemon: starts the MCP stdio server and all background services.
    Daemon,
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
    /// Install so-context as an MCP server in Claude, OpenCode, and Codex configs.
    Setup {
        /// Path to the so-context binary (default: current executable).
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
        Commands::Index { path } => {
            let output = core_graph::index_project(&path).map_err(anyhow::Error::msg)?;
            println!("{output}");
            Ok(())
        }
        Commands::Watch { path } => {
            core_graph::watch_project(&path).map_err(anyhow::Error::msg)
        }
        Commands::Setup { binary } => {
            let bin = match binary {
                Some(b) => b,
                None => std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "so-context".to_string()),
            };
            setup::install_all(&bin)
        }
    }
}

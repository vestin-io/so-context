#[path = "core/read.rs"]
mod core_read;
#[path = "core/graph/mod.rs"]
mod core_graph;
mod mcp;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "so-context", version, about = "CLI with built-in MCP daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the built-in standard MCP server over stdio.
    Daemon,
    /// Index a project folder into the local code graph SQLite database.
    Index {
        /// Project folder path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
    },
    /// Index a project folder and keep watching for file changes.
    Watch {
        /// Project folder path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
    },
}

/// Parses CLI arguments and dispatches to daemon/index/watch execution paths.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Daemon => mcp::run_stdio_server().await,
        Commands::Index { path } => {
            let output = core_graph::index_project(&path)
                .map_err(anyhow::Error::msg)?;
            println!("{output}");
            Ok(())
        }
        Commands::Watch { path } => core_graph::watch_project(&path).map_err(anyhow::Error::msg),
    }
}

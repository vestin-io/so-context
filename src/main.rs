#[path = "core/read.rs"]
mod core_read;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Daemon => mcp::run_stdio_server().await,
    }
}

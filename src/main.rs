#[path = "core/graph/mod.rs"]
mod core_graph;
#[path = "core/read.rs"]
mod core_read;
mod daemon;
mod mcp;
mod shell;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process;

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
    /// Execute a shell command and print a deterministic compressed summary.
    Shell {
        /// Print raw command output instead of the compressed shell view.
        #[arg(long)]
        full: bool,
        /// Persist raw stdout/stderr for later compression analysis.
        #[arg(long)]
        record_raw: bool,
        /// Include shell classifier and execution metadata in the output.
        #[arg(long)]
        debug_shell: bool,
        /// Command and arguments to execute.
        #[arg(required = true, num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
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
        Commands::Watch { path } => core_graph::watch_project(&path).map_err(anyhow::Error::msg),
        Commands::Shell {
            full,
            record_raw,
            debug_shell,
            argv,
        } => {
            let output = shell::run(&argv, full, record_raw, debug_shell)?;
            print!("{}", output.rendered);
            if output.exit_code != 0 {
                process::exit(output.exit_code);
            }
            Ok(())
        }
    }
}

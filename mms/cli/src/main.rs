mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "mms")]
#[command(author = "MMS Team")]
#[command(version = "0.1.0")]
#[command(about = "MMS - AI Agent CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Run the agent")]
    Run {
        #[command(subcommand)]
        mode: RunMode,
    },
    #[command(about = "Manage MCP servers")]
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    #[command(about = "Show configuration")]
    Config,
}

#[derive(Subcommand)]
enum RunMode {
    #[command(about = "Run in CLI mode (terminal)")]
    Cli,
    #[command(about = "Run in Web mode (Tauri app)")]
    Web,
}

#[derive(Subcommand)]
enum McpAction {
    #[command(about = "List configured MCP servers")]
    List,
    #[command(about = "Add an MCP server")]
    Add {
        #[arg(help = "Server name")]
        name: String,
        #[arg(help = "Command to run")]
        command: String,
    },
    #[command(about = "Remove an MCP server")]
    Remove {
        #[arg(help = "Server name")]
        name: String,
    },
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { mode } => match mode {
            RunMode::Cli => commands::run_cli().await,
            RunMode::Web => commands::run_web().await,
        },
        Commands::Mcp { action } => match action {
            McpAction::List => commands::mcp_list().await,
            McpAction::Add { name, command } => commands::mcp_add(&name, &command).await,
            McpAction::Remove { name } => commands::mcp_remove(&name).await,
        },
        Commands::Config => commands::show_config().await,
    }
}

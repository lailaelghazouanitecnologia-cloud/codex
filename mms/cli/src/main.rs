mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
    #[command(about = "Execute a shell command with sandbox")]
    Shell {
        #[arg(help = "Command to execute")]
        command: String,
        #[arg(short, long, help = "Path to exec policy file")]
        policy: Option<PathBuf>,
        #[arg(long, help = "Disable sandbox (run without restrictions)")]
        no_sandbox: bool,
        #[arg(long, help = "Allow network access")]
        allow_network: bool,
        #[arg(long, help = "Allow full disk write access")]
        allow_write: bool,
        #[arg(short, long, help = "Timeout in milliseconds", default_value = "30000")]
        timeout: u64,
    },
    #[command(about = "Manage MCP servers")]
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    #[command(about = "Manage execution policies")]
    Policy {
        #[command(subcommand)]
        action: PolicyAction,
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

#[derive(Subcommand)]
enum PolicyAction {
    #[command(about = "Validate a policy file")]
    Validate {
        #[arg(help = "Path to policy file")]
        path: PathBuf,
    },
    #[command(about = "Test a command against a policy")]
    Test {
        #[arg(help = "Command to test")]
        command: String,
        #[arg(short, long, help = "Path to policy file (uses heuristics if not provided)")]
        policy: Option<PathBuf>,
    },
    #[command(about = "Show policy information and defaults")]
    Info,
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
        Commands::Shell {
            command,
            policy,
            no_sandbox,
            allow_network,
            allow_write,
            timeout,
        } => {
            commands::shell_exec(
                command,
                policy,
                no_sandbox,
                allow_network,
                allow_write,
                timeout,
            )
            .await
        }
        Commands::Mcp { action } => match action {
            McpAction::List => commands::mcp_list().await,
            McpAction::Add { name, command } => commands::mcp_add(&name, &command).await,
            McpAction::Remove { name } => commands::mcp_remove(&name).await,
        },
        Commands::Policy { action } => match action {
            PolicyAction::Validate { path } => commands::policy_validate(path).await,
            PolicyAction::Test { command, policy } => commands::policy_test(policy, command).await,
            PolicyAction::Info => commands::policy_info().await,
        },
        Commands::Config => commands::show_config().await,
    }
}

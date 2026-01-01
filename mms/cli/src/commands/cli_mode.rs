use std::io::{self, BufRead, Write};

use anyhow::Result;
use mms_config::ConfigLoader;
use mms_core::Agent;
use mms_protocol::EventMessage;
use tracing::info;

pub async fn run_cli() -> Result<()> {
    info!("Starting MMS in CLI mode");

    let loader = ConfigLoader::from_default_home()?;
    let config = loader.load()?;

    let agent = Agent::new();
    let session_id = agent.start(config).await?;

    println!("MMS Agent Started");
    println!("Session: {}", session_id);
    println!("Type your message or 'exit' to quit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut input = String::new();
        if stdin.lock().read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }

        agent.send_message(input).await?;

        loop {
            let event = agent.next_event().await?;

            match event.message {
                EventMessage::AgentThinking(_) => {
                    print!(".");
                    stdout.flush()?;
                }
                EventMessage::AgentMessage(msg) => {
                    println!("\n{}", msg.content);
                    break;
                }
                EventMessage::ToolCallStarted(call) => {
                    println!("\n[Tool: {}]", call.tool_name);
                }
                EventMessage::ToolCallCompleted(result) => {
                    println!("[Tool completed: {:?}]", result.result);
                }
                EventMessage::TurnCompleted(_) => {
                    break;
                }
                EventMessage::ShutdownComplete => {
                    println!("Agent shutdown");
                    break;
                }
                _ => {}
            }
        }
    }

    agent.shutdown().await?;
    Ok(())
}

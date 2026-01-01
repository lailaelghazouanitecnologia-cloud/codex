use mms_common::{AgentError, AgentResult};
use mms_config::Config;
use mms_protocol::{
    ApprovalRequiredEvent, EventMessage, RiskLevel, ToolCallCompletedEvent,
    ToolCallStartedEvent, ToolResult, TurnCompletedEvent,
};
use mms_tools::{ToolCall, ToolContext, ToolOutput, ToolRouter};
use async_channel::Sender;
use std::sync::Arc;

use crate::turn::{Turn, TurnState};

pub struct Executor {
    config: Arc<Config>,
    router: Arc<ToolRouter>,
    event_tx: Sender<EventMessage>,
}

impl Executor {
    pub fn new(
        config: Arc<Config>,
        router: Arc<ToolRouter>,
        event_tx: Sender<EventMessage>,
    ) -> Self {
        Self {
            config,
            router,
            event_tx,
        }
    }

    pub async fn execute_tool_calls(
        &self,
        turn: &mut Turn,
        calls: Vec<ToolCall>,
    ) -> AgentResult<Vec<ToolOutput>> {
        let mut outputs = Vec::with_capacity(calls.len());

        for call in calls {
            let output = self.execute_single_call(turn, call).await?;
            outputs.push(output);
        }

        Ok(outputs)
    }

    async fn execute_single_call(
        &self,
        turn: &mut Turn,
        call: ToolCall,
    ) -> AgentResult<ToolOutput> {
        self.emit(EventMessage::ToolCallStarted(ToolCallStartedEvent {
            call_id: call.id.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        }))
        .await?;

        if self.requires_approval(&call) {
            turn.set_state(TurnState::WaitingApproval);

            self.emit(EventMessage::ApprovalRequired(ApprovalRequiredEvent {
                request_id: call.id.clone(),
                tool_name: call.name.clone(),
                description: format!("Execute: {}", call.name),
                risk_level: self.assess_risk(&call),
            }))
            .await?;

            return Ok(ToolOutput::error(
                call.id,
                "Awaiting approval",
                0,
            ));
        }

        turn.set_state(TurnState::Executing);

        let ctx = ToolContext::new(self.config.clone());
        let output = self.router.execute(&ctx, call.clone()).await?;

        let result = if output.success {
            ToolResult::Success {
                output: output.content.clone(),
            }
        } else {
            ToolResult::Error {
                message: output.content.clone(),
            }
        };

        self.emit(EventMessage::ToolCallCompleted(ToolCallCompletedEvent {
            call_id: output.call_id.clone(),
            tool_name: call.name,
            result,
            duration_ms: output.duration_ms,
        }))
        .await?;

        Ok(output)
    }

    fn requires_approval(&self, call: &ToolCall) -> bool {
        use mms_protocol::ApprovalMode;

        match self.config.approval_mode {
            ApprovalMode::Always => true,
            ApprovalMode::Never => false,
            ApprovalMode::OnDanger => self.router.is_dangerous(call),
        }
    }

    fn assess_risk(&self, call: &ToolCall) -> RiskLevel {
        match call.name.as_str() {
            "shell" => RiskLevel::High,
            "write_file" => RiskLevel::Medium,
            "read_file" => RiskLevel::Low,
            _ => RiskLevel::Medium,
        }
    }

    pub async fn complete_turn(&self, turn: &Turn) -> AgentResult<()> {
        self.emit(EventMessage::TurnCompleted(TurnCompletedEvent {
            input_tokens: turn.input_tokens,
            output_tokens: turn.output_tokens,
            duration_ms: turn.duration_ms(),
        }))
        .await
    }

    async fn emit(&self, message: EventMessage) -> AgentResult<()> {
        self.event_tx
            .send(message)
            .await
            .map_err(|_| AgentError::ChannelClosed)
    }
}

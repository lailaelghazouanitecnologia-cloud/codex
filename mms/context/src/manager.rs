use mms_providers::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::truncation::{ContextMessage, TruncationStrategy};
use crate::window::ContextWindow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub strategy: TruncationStrategy,
    pub reserve_for_response: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128000,
            strategy: TruncationStrategy::Head,
            reserve_for_response: 4096,
        }
    }
}

pub struct ContextManager {
    windows: HashMap<String, ContextWindow>,
    config: ContextConfig,
}

impl ContextManager {
    pub fn new(config: ContextConfig) -> Self {
        Self {
            windows: HashMap::new(),
            config,
        }
    }

    pub fn create_window(&mut self, session_id: impl Into<String>) -> &mut ContextWindow {
        let id = session_id.into();
        let effective_max = self.config.max_tokens.saturating_sub(self.config.reserve_for_response);
        let window = ContextWindow::new(effective_max).with_strategy(self.config.strategy);

        self.windows.entry(id.clone()).or_insert(window);
        self.windows.get_mut(&id).unwrap()
    }

    pub fn get_window(&self, session_id: &str) -> Option<&ContextWindow> {
        self.windows.get(session_id)
    }

    pub fn get_window_mut(&mut self, session_id: &str) -> Option<&mut ContextWindow> {
        self.windows.get_mut(session_id)
    }

    pub fn remove_window(&mut self, session_id: &str) -> Option<ContextWindow> {
        self.windows.remove(session_id)
    }

    pub fn add_message(&mut self, session_id: &str, message: ContextMessage) -> bool {
        if let Some(window) = self.windows.get_mut(session_id) {
            window.add(message);
            return true;
        }
        false
    }

    pub fn convert_from_provider_messages(&self, messages: &[Message]) -> Vec<ContextMessage> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    mms_providers::MessageRole::System => "system",
                    mms_providers::MessageRole::User => "user",
                    mms_providers::MessageRole::Assistant => "assistant",
                    mms_providers::MessageRole::Tool => "tool",
                };

                let content = match &m.content {
                    mms_providers::MessageContent::Text(text) => text.clone(),
                    mms_providers::MessageContent::Parts(_) => String::new(),
                };

                ContextMessage::new(content, role)
            })
            .collect()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn total_tokens(&self) -> usize {
        self.windows.values().map(|w| w.current_tokens()).sum()
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(ContextConfig::default())
    }
}

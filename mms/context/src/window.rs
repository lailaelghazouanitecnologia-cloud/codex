use serde::{Deserialize, Serialize};

use crate::truncation::{ContextMessage, TruncationStrategy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    messages: Vec<ContextMessage>,
    max_tokens: usize,
    current_tokens: usize,
    strategy: TruncationStrategy,
}

impl ContextWindow {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_tokens,
            current_tokens: 0,
            strategy: TruncationStrategy::default(),
        }
    }

    pub fn with_strategy(mut self, strategy: TruncationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn add(&mut self, message: ContextMessage) {
        self.current_tokens += message.estimated_tokens;
        self.messages.push(message);

        if self.current_tokens > self.max_tokens {
            self.truncate();
        }
    }

    pub fn add_message(&mut self, content: impl Into<String>, role: impl Into<String>) {
        let message = ContextMessage::new(content, role);
        self.add(message);
    }

    fn truncate(&mut self) {
        self.messages = self.strategy.truncate(&self.messages, self.max_tokens);
        self.recalculate_tokens();
    }

    fn recalculate_tokens(&mut self) {
        self.current_tokens = self.messages.iter().map(|m| m.estimated_tokens).sum();
    }

    pub fn messages(&self) -> &[ContextMessage] {
        &self.messages
    }

    pub fn current_tokens(&self) -> usize {
        self.current_tokens
    }

    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    pub fn remaining_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.current_tokens)
    }

    pub fn is_full(&self) -> bool {
        self.current_tokens >= self.max_tokens
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.current_tokens = 0;
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for ContextWindow {
    fn default() -> Self {
        Self::new(128000)
    }
}

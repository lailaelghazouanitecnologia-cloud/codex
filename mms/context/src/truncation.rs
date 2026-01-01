use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruncationStrategy {
    Head,
    Tail,
    Middle,
    Summarize,
}

impl TruncationStrategy {
    pub fn truncate(&self, messages: &[ContextMessage], max_tokens: usize) -> Vec<ContextMessage> {
        let total_tokens: usize = messages.iter().map(|m| m.estimated_tokens).sum();

        if total_tokens <= max_tokens {
            return messages.to_vec();
        }

        let tokens_to_remove = total_tokens - max_tokens;

        match self {
            Self::Head => self.truncate_head(messages, tokens_to_remove),
            Self::Tail => self.truncate_tail(messages, tokens_to_remove),
            Self::Middle => self.truncate_middle(messages, tokens_to_remove),
            Self::Summarize => messages.to_vec(),
        }
    }

    fn truncate_head(&self, messages: &[ContextMessage], tokens_to_remove: usize) -> Vec<ContextMessage> {
        let mut removed = 0;
        let mut start_index = 0;

        for (i, msg) in messages.iter().enumerate() {
            if msg.is_system {
                continue;
            }
            removed += msg.estimated_tokens;
            start_index = i + 1;
            if removed >= tokens_to_remove {
                break;
            }
        }

        let mut result: Vec<ContextMessage> = messages
            .iter()
            .filter(|m| m.is_system)
            .cloned()
            .collect();

        result.extend(messages[start_index..].iter().cloned());
        result
    }

    fn truncate_tail(&self, messages: &[ContextMessage], tokens_to_remove: usize) -> Vec<ContextMessage> {
        let mut removed = 0;
        let mut end_index = messages.len();

        for (i, msg) in messages.iter().enumerate().rev() {
            removed += msg.estimated_tokens;
            end_index = i;
            if removed >= tokens_to_remove {
                break;
            }
        }

        messages[..end_index].to_vec()
    }

    fn truncate_middle(&self, messages: &[ContextMessage], tokens_to_remove: usize) -> Vec<ContextMessage> {
        let non_system: Vec<_> = messages.iter().filter(|m| !m.is_system).collect();
        let mid = non_system.len() / 2;

        let mut removed = 0;
        let mut remove_indices = Vec::new();

        for offset in 0..non_system.len() {
            let forward_idx = mid + offset / 2;
            let backward_idx = mid.saturating_sub((offset + 1) / 2);

            if offset % 2 == 0 && forward_idx < non_system.len() {
                removed += non_system[forward_idx].estimated_tokens;
                remove_indices.push(forward_idx);
            } else if backward_idx < mid {
                removed += non_system[backward_idx].estimated_tokens;
                remove_indices.push(backward_idx);
            }

            if removed >= tokens_to_remove {
                break;
            }
        }

        messages
            .iter()
            .enumerate()
            .filter(|(i, m)| m.is_system || !remove_indices.contains(i))
            .map(|(_, m)| m.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub content: String,
    pub role: String,
    pub estimated_tokens: usize,
    pub is_system: bool,
}

impl ContextMessage {
    pub fn new(content: impl Into<String>, role: impl Into<String>) -> Self {
        let content = content.into();
        let role_str = role.into();
        let estimated_tokens = content.len() / 4;
        let is_system = role_str == "system";

        Self {
            content,
            role: role_str,
            estimated_tokens,
            is_system,
        }
    }
}

impl Default for TruncationStrategy {
    fn default() -> Self {
        Self::Head
    }
}

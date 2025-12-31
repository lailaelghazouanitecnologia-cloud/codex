use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    UserMessage(UserMessageOp),
    Approve(ApproveOp),
    Reject(RejectOp),
    Interrupt,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageOp {
    pub content: String,
    pub images: Vec<PathBuf>,
    pub cwd: Option<PathBuf>,
}

impl UserMessageOp {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            images: Vec::new(),
            cwd: None,
        }
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    pub fn with_images(mut self, images: Vec<PathBuf>) -> Self {
        self.images = images;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveOp {
    pub request_id: String,
}

impl ApproveOp {
    pub fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectOp {
    pub request_id: String,
    pub reason: Option<String>,
}

impl RejectOp {
    pub fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

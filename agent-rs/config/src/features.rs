use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Feature {
    ShellTool,
    FileReadTool,
    FileWriteTool,
    WebSearch,
    ParallelTools,
    Streaming,
}

impl Feature {
    pub fn key(&self) -> &'static str {
        match self {
            Self::ShellTool => "shell_tool",
            Self::FileReadTool => "file_read_tool",
            Self::FileWriteTool => "file_write_tool",
            Self::WebSearch => "web_search",
            Self::ParallelTools => "parallel_tools",
            Self::Streaming => "streaming",
        }
    }

    pub fn default_enabled(&self) -> bool {
        match self {
            Self::ShellTool => true,
            Self::FileReadTool => true,
            Self::FileWriteTool => true,
            Self::WebSearch => false,
            Self::ParallelTools => true,
            Self::Streaming => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    enabled: BTreeSet<Feature>,
}

impl Features {
    pub fn new() -> Self {
        Self {
            enabled: BTreeSet::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut enabled = BTreeSet::new();
        for feature in Self::all_features() {
            if feature.default_enabled() {
                enabled.insert(*feature);
            }
        }
        Self { enabled }
    }

    pub fn all_features() -> &'static [Feature] {
        &[
            Feature::ShellTool,
            Feature::FileReadTool,
            Feature::FileWriteTool,
            Feature::WebSearch,
            Feature::ParallelTools,
            Feature::Streaming,
        ]
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.enabled.contains(&feature)
    }

    pub fn enable(&mut self, feature: Feature) -> &mut Self {
        self.enabled.insert(feature);
        self
    }

    pub fn disable(&mut self, feature: Feature) -> &mut Self {
        self.enabled.remove(&feature);
        self
    }

    pub fn set(&mut self, feature: Feature, enabled: bool) -> &mut Self {
        if enabled {
            self.enable(feature)
        } else {
            self.disable(feature)
        }
    }
}

impl Default for Features {
    fn default() -> Self {
        Self::with_defaults()
    }
}

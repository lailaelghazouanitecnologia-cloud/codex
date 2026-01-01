use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: usize,
    pub column: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

impl SearchMatch {
    pub fn new(
        line_number: usize,
        column: usize,
        line_content: impl Into<String>,
        match_start: usize,
        match_end: usize,
    ) -> Self {
        Self {
            line_number,
            column,
            line_content: line_content.into(),
            match_start,
            match_end,
        }
    }

    pub fn matched_text(&self) -> &str {
        &self.line_content[self.match_start..self.match_end.min(self.line_content.len())]
    }

    pub fn context(&self, chars_before: usize, chars_after: usize) -> String {
        let start = self.match_start.saturating_sub(chars_before);
        let end = (self.match_end + chars_after).min(self.line_content.len());
        self.line_content[start..end].to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: PathBuf,
    pub matches: Vec<SearchMatch>,
    pub file_type: Option<String>,
    pub size_bytes: u64,
}

impl SearchResult {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            matches: Vec::new(),
            file_type: None,
            size_bytes: 0,
        }
    }

    pub fn with_match(mut self, m: SearchMatch) -> Self {
        self.matches.push(m);
        self
    }

    pub fn with_matches(mut self, matches: Vec<SearchMatch>) -> Self {
        self.matches = matches;
        self
    }

    pub fn with_file_info(mut self, file_type: Option<String>, size_bytes: u64) -> Self {
        self.file_type = file_type;
        self.size_bytes = size_bytes;
        self
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSummary {
    pub total_files_searched: usize,
    pub files_with_matches: usize,
    pub total_matches: usize,
    pub duration_ms: u64,
}

impl SearchSummary {
    pub fn new() -> Self {
        Self {
            total_files_searched: 0,
            files_with_matches: 0,
            total_matches: 0,
            duration_ms: 0,
        }
    }

    pub fn add_result(&mut self, result: &SearchResult) {
        self.total_files_searched += 1;
        if result.has_matches() {
            self.files_with_matches += 1;
            self.total_matches += result.match_count();
        }
    }
}

impl Default for SearchSummary {
    fn default() -> Self {
        Self::new()
    }
}

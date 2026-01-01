use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchPattern {
    Exact(String),
    Glob(String),
    Regex(String),
    Fuzzy(String),
}

impl SearchPattern {
    pub fn exact(pattern: impl Into<String>) -> Self {
        Self::Exact(pattern.into())
    }

    pub fn glob(pattern: impl Into<String>) -> Self {
        Self::Glob(pattern.into())
    }

    pub fn regex(pattern: impl Into<String>) -> Self {
        Self::Regex(pattern.into())
    }

    pub fn fuzzy(pattern: impl Into<String>) -> Self {
        Self::Fuzzy(pattern.into())
    }

    pub fn matches(&self, text: &str) -> bool {
        match self {
            Self::Exact(p) => text.contains(p),
            Self::Glob(p) => self.match_glob(p, text),
            Self::Regex(p) => self.match_regex(p, text),
            Self::Fuzzy(p) => self.match_fuzzy(p, text),
        }
    }

    fn match_glob(&self, pattern: &str, text: &str) -> bool {
        let regex_pattern = pattern
            .replace('.', "\\.")
            .replace('*', ".*")
            .replace('?', ".");

        if let Ok(re) = Regex::new(&regex_pattern) {
            return re.is_match(text);
        }
        false
    }

    fn match_regex(&self, pattern: &str, text: &str) -> bool {
        if let Ok(re) = Regex::new(pattern) {
            return re.is_match(text);
        }
        false
    }

    fn match_fuzzy(&self, pattern: &str, text: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        let text_lower = text.to_lowercase();

        let mut pattern_chars = pattern_lower.chars().peekable();

        for text_char in text_lower.chars() {
            if let Some(&pattern_char) = pattern_chars.peek() {
                if text_char == pattern_char {
                    pattern_chars.next();
                }
            }
        }

        pattern_chars.peek().is_none()
    }

    pub fn pattern_string(&self) -> &str {
        match self {
            Self::Exact(p) | Self::Glob(p) | Self::Regex(p) | Self::Fuzzy(p) => p,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub include_hidden: bool,
    pub max_depth: Option<usize>,
    pub max_results: Option<usize>,
    pub file_extensions: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl SearchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn case_sensitive(mut self, value: bool) -> Self {
        self.case_sensitive = value;
        self
    }

    pub fn include_hidden(mut self, value: bool) -> Self {
        self.include_hidden = value;
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn max_results(mut self, count: usize) -> Self {
        self.max_results = Some(count);
        self
    }

    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.file_extensions = extensions;
        self
    }

    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            include_hidden: false,
            max_depth: None,
            max_results: Some(100),
            file_extensions: Vec::new(),
            exclude_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
            ],
        }
    }
}

use mms_common::AgentResult;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::pattern::{SearchOptions, SearchPattern};
use crate::result::{SearchMatch, SearchResult, SearchSummary};

pub struct FileSearcher {
    options: SearchOptions,
}

impl FileSearcher {
    pub fn new() -> Self {
        Self {
            options: SearchOptions::default(),
        }
    }

    pub fn with_options(options: SearchOptions) -> Self {
        Self { options }
    }

    pub fn search_files(&self, root: &Path, pattern: &SearchPattern) -> AgentResult<Vec<SearchResult>> {
        let start = std::time::Instant::now();
        let mut results = Vec::new();
        let mut count = 0;

        let walker = self.create_walker(root);

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            if !self.should_search_file(path) {
                continue;
            }

            if let Ok(result) = self.search_file(path, pattern) {
                if result.has_matches() {
                    results.push(result);
                    count += 1;

                    if let Some(max) = self.options.max_results {
                        if count >= max {
                            break;
                        }
                    }
                }
            }
        }

        let _duration = start.elapsed();
        Ok(results)
    }

    pub fn search_file(&self, path: &Path, pattern: &SearchPattern) -> AgentResult<SearchResult> {
        let content = fs::read_to_string(path)
            .map_err(|e| mms_common::AgentError::io_error(e.to_string()))?;

        let metadata = fs::metadata(path).ok();
        let size_bytes = metadata.map(|m| m.len()).unwrap_or(0);
        let file_type = path.extension().and_then(|e| e.to_str()).map(String::from);

        let matches = self.find_matches(&content, pattern);

        Ok(SearchResult::new(path.to_path_buf())
            .with_matches(matches)
            .with_file_info(file_type, size_bytes))
    }

    fn create_walker(&self, root: &Path) -> walkdir::IntoIter {
        let mut walker = WalkDir::new(root);

        if let Some(depth) = self.options.max_depth {
            walker = walker.max_depth(depth);
        }

        walker.into_iter()
    }

    fn should_search_file(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !self.options.include_hidden && file_name.starts_with('.') {
            return false;
        }

        for exclude in &self.options.exclude_patterns {
            if path.to_string_lossy().contains(exclude) {
                return false;
            }
        }

        if !self.options.file_extensions.is_empty() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !self.options.file_extensions.iter().any(|e| e == ext) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    fn find_matches(&self, content: &str, pattern: &SearchPattern) -> Vec<SearchMatch> {
        let mut matches = Vec::new();

        let search_content = if self.options.case_sensitive {
            content.to_string()
        } else {
            content.to_lowercase()
        };

        let search_pattern = if self.options.case_sensitive {
            pattern.pattern_string().to_string()
        } else {
            pattern.pattern_string().to_lowercase()
        };

        for (line_num, line) in content.lines().enumerate() {
            let search_line = if self.options.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };

            if let Some(start) = search_line.find(&search_pattern) {
                let end = start + search_pattern.len();
                matches.push(SearchMatch::new(line_num + 1, start + 1, line, start, end));
            }
        }

        matches
    }

    pub fn search_with_regex(&self, root: &Path, regex_pattern: &str) -> AgentResult<Vec<SearchResult>> {
        let re = Regex::new(regex_pattern)
            .map_err(|e| mms_common::AgentError::parse(e.to_string()))?;

        let mut results = Vec::new();
        let walker = self.create_walker(root);

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            if !self.should_search_file(path) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(path) {
                let matches = self.find_regex_matches(&content, &re);
                if !matches.is_empty() {
                    results.push(SearchResult::new(path.to_path_buf()).with_matches(matches));
                }
            }
        }

        Ok(results)
    }

    fn find_regex_matches(&self, content: &str, re: &Regex) -> Vec<SearchMatch> {
        let mut matches = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for m in re.find_iter(line) {
                matches.push(SearchMatch::new(
                    line_num + 1,
                    m.start() + 1,
                    line,
                    m.start(),
                    m.end(),
                ));
            }
        }

        matches
    }

    pub fn glob_files(&self, root: &Path, glob_pattern: &str) -> AgentResult<Vec<PathBuf>> {
        let pattern = glob::Pattern::new(glob_pattern)
            .map_err(|e| mms_common::AgentError::parse(e.to_string()))?;

        let mut results = Vec::new();
        let walker = self.create_walker(root);

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            if pattern.matches_path(path) {
                results.push(path.to_path_buf());
            }
        }

        Ok(results)
    }
}

impl Default for FileSearcher {
    fn default() -> Self {
        Self::new()
    }
}

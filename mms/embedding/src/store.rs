use std::collections::HashMap;

use crate::vector::Embedding;

#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub embedding: Embedding,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VectorEntry {
    pub fn new(id: impl Into<String>, embedding: Embedding) -> Self {
        Self {
            id: id.into(),
            embedding,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: VectorEntry,
    pub score: f32,
}

pub struct VectorStore {
    entries: Vec<VectorEntry>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, entry: VectorEntry) {
        self.entries.push(entry);
    }

    pub fn insert_batch(&mut self, entries: Vec<VectorEntry>) {
        self.entries.extend(entries);
    }

    pub fn search(&self, query: &Embedding, top_k: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = self
            .entries
            .iter()
            .map(|entry| SearchResult {
                entry: entry.clone(),
                score: query.cosine_similarity(&entry.embedding),
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    pub fn search_with_threshold(
        &self,
        query: &Embedding,
        threshold: f32,
        top_k: usize,
    ) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = self
            .entries
            .iter()
            .map(|entry| SearchResult {
                entry: entry.clone(),
                score: query.cosine_similarity(&entry.embedding),
            })
            .filter(|r| r.score >= threshold)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < initial_len
    }

    pub fn get(&self, id: &str) -> Option<&VectorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for VectorStore {
    fn default() -> Self {
        Self::new()
    }
}

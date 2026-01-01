use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub values: Vec<f32>,
    pub dimensions: usize,
}

impl Embedding {
    pub fn new(values: Vec<f32>) -> Self {
        let dimensions = values.len();
        Self { values, dimensions }
    }

    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        if self.dimensions != other.dimensions {
            return 0.0;
        }

        let dot_product: f32 = self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum();

        let magnitude_a: f32 = self.values.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = other.values.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }

    pub fn euclidean_distance(&self, other: &Embedding) -> f32 {
        if self.dimensions != other.dimensions {
            return f32::MAX;
        }

        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub input: Vec<String>,
    pub model: String,
}

impl EmbeddingRequest {
    pub fn new(input: Vec<String>, model: impl Into<String>) -> Self {
        Self {
            input,
            model: model.into(),
        }
    }

    pub fn single(text: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            input: vec![text.into()],
            model: model.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Embedding>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

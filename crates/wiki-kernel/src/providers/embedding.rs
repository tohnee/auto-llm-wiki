use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wiki_core::RankedClaim;
use wiki_storage::{SqliteWikiRepository, StoredEmbedding};

pub type EmbeddingResult<T> = std::result::Result<T, EmbeddingError>;

pub trait EmbeddingProvider {
    fn embed_text(&self, input: &str) -> EmbeddingResult<Vec<f32>>;
}

pub struct OpenAiCompatibleEmbeddingClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiCompatibleEmbeddingClient {
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        model: String,
        timeout_ms: u64,
    ) -> EmbeddingResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()?;
        Ok(Self {
            client,
            base_url,
            api_key,
            model,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl EmbeddingProvider for OpenAiCompatibleEmbeddingClient {
    fn embed_text(&self, input: &str) -> EmbeddingResult<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let mut request = self.client.post(url).json(&EmbeddingRequest {
            model: self.model.clone(),
            input: input.to_owned(),
        });
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request.send()?.error_for_status()?;
        let payload: EmbeddingResponse = response.json()?;
        payload
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .ok_or(EmbeddingError::MissingEmbedding)
    }
}

pub struct CosineVectorRetriever<'a, P> {
    repo: &'a SqliteWikiRepository,
    provider: &'a P,
    model: String,
    top_k: usize,
}

impl<'a, P> CosineVectorRetriever<'a, P>
where
    P: EmbeddingProvider,
{
    pub fn new(repo: &'a SqliteWikiRepository, provider: &'a P, model: &str, top_k: usize) -> Self {
        Self {
            repo,
            provider,
            model: model.to_owned(),
            top_k,
        }
    }

    pub fn retrieve(&self, query: &str) -> EmbeddingResult<Vec<RankedClaim>> {
        let query_vector = self.provider.embed_text(query)?;
        let embeddings = self.repo.list_ready_embeddings_by_model(&self.model)?;
        let mut scored: Vec<(f64, StoredEmbedding)> = embeddings
            .into_iter()
            .map(|embedding| (cosine_similarity(&query_vector, &embedding.vector), embedding))
            .filter(|(score, _)| *score > 0.0)
            .collect();
        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        scored.truncate(self.top_k);
        Ok(scored
            .into_iter()
            .enumerate()
            .map(|(index, (_, embedding))| RankedClaim::new(embedding.claim_id, index + 1))
            .collect())
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f64 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let dot: f64 = left
        .iter()
        .zip(right.iter())
        .map(|(l, r)| (*l as f64) * (*r as f64))
        .sum();
    let left_norm: f64 = left.iter().map(|value| (*value as f64).powi(2)).sum::<f64>().sqrt();
    let right_norm: f64 = right.iter().map(|value| (*value as f64).powi(2)).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("storage error: {0}")]
    Storage(#[from] wiki_storage::StorageError),
    #[error("embedding response did not contain any vectors")]
    MissingEmbedding,
}

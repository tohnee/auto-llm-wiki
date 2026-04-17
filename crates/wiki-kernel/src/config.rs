use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub retrieval: RetrievalConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalConfig {
    pub keyword: KeywordConfig,
    pub vector: VectorConfig,
    pub graph: GraphConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordConfig {
    pub enabled: bool,
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_ms: u64,
    pub batch_size: usize,
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphConfig {
    pub enabled: bool,
    pub walk_depth: usize,
    pub max_neighbors: usize,
    pub top_k: usize,
}

impl RuntimeConfig {
    pub fn vector_enabled_for_tests(model: &str) -> Self {
        Self {
            retrieval: RetrievalConfig {
                keyword: KeywordConfig {
                    enabled: true,
                    top_k: 20,
                },
                vector: VectorConfig {
                    enabled: true,
                    base_url: "http://localhost".to_owned(),
                    api_key: None,
                    model: model.to_owned(),
                    timeout_ms: 5_000,
                    batch_size: 16,
                    top_k: 20,
                },
                graph: GraphConfig {
                    enabled: true,
                    walk_depth: 2,
                    max_neighbors: 32,
                    top_k: 20,
                },
            },
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            retrieval: RetrievalConfig {
                keyword: KeywordConfig {
                    enabled: true,
                    top_k: 20,
                },
                vector: VectorConfig {
                    enabled: false,
                    base_url: String::new(),
                    api_key: None,
                    model: "embedding-small".to_owned(),
                    timeout_ms: 30_000,
                    batch_size: 16,
                    top_k: 20,
                },
                graph: GraphConfig {
                    enabled: true,
                    walk_depth: 2,
                    max_neighbors: 32,
                    top_k: 20,
                },
            },
        }
    }
}

pub fn load_runtime_config(path: impl AsRef<Path>) -> Result<RuntimeConfig> {
    let raw = fs::read_to_string(path)?;
    let mut config: RuntimeConfig = toml::from_str(&raw)?;

    config.retrieval.vector.api_key = config
        .retrieval
        .vector
        .api_key
        .as_deref()
        .map(resolve_secret)
        .transpose()?;

    Ok(config)
}

fn resolve_secret(raw: &str) -> Result<String> {
    if let Some(key) = raw.strip_prefix("env:") {
        return env::var(key).map_err(|_| ConfigError::MissingEnvVar(key.to_owned()));
    }
    Ok(raw.to_owned())
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("missing environment variable: {0}")]
    MissingEnvVar(String),
}

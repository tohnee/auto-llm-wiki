mod config;
mod engine;
mod lint;
mod providers;
mod retrieval;
mod wiki;

pub use config::{
    load_runtime_config, ConfigError, GraphConfig, KeywordConfig, RetrievalConfig, RuntimeConfig,
    VectorConfig,
};
pub use engine::{KernelError, QueryOptions, QueryResult, WikiEngine};
pub use providers::embedding::{
    CosineVectorRetriever, EmbeddingError, EmbeddingProvider, EmbeddingResult,
    OpenAiCompatibleEmbeddingClient,
};
pub use providers::graph::MempalaceGraphRetriever;
pub use providers::keyword::{KeywordRetriever, SqliteFtsRetriever};

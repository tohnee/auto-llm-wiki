use std::path::{Path, PathBuf};

use chrono::Utc;
use sha2::{Digest, Sha256};
use thiserror::Error;
use wiki_core::{
    AuditAction, Claim, ClaimId, ClaimReplacement, HealthStatus, LintIssue, MemoryTier,
    OutboxEvent, OutboxEventKind, ProviderHealth,
};
use wiki_storage::{SqliteWikiRepository, StorageError};
use wiki_mempalace_bridge::MempalaceGraphBridge;

use crate::{
    config::RuntimeConfig,
    lint::{render_report, run_lint},
    providers::embedding::{CosineVectorRetriever, EmbeddingError, EmbeddingProvider},
    providers::graph::MempalaceGraphRetriever,
    providers::keyword::{KeywordRetriever, SqliteFtsRetriever},
    retrieval::retrieve_claims,
    wiki::{ensure_layout, slugify, write_page, write_report},
};

pub type Result<T> = std::result::Result<T, KernelError>;

pub struct WikiEngine {
    repo: SqliteWikiRepository,
    wiki_dir: PathBuf,
    runtime_config: RuntimeConfig,
}

#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub write_page: bool,
    pub page_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub claims: Vec<Claim>,
    pub page_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SyncIndexResult {
    pub embedded_claims: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
}

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("claim not found: {0}")]
    ClaimNotFound(ClaimId),
    #[error("embedding provider error: {0}")]
    Embedding(#[from] EmbeddingError),
}

impl WikiEngine {
    pub fn new(repo: SqliteWikiRepository, wiki_dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_config(repo, wiki_dir, RuntimeConfig::default())
    }

    pub fn with_config(
        repo: SqliteWikiRepository,
        wiki_dir: impl AsRef<Path>,
        runtime_config: RuntimeConfig,
    ) -> Result<Self> {
        let wiki_dir = wiki_dir.as_ref().to_path_buf();
        ensure_layout(&wiki_dir)?;
        Ok(Self {
            repo,
            wiki_dir,
            runtime_config,
        })
    }

    pub fn ingest(&self, actor: &str, source_uri: &str, content: &str, scope: &str) -> Result<Claim> {
        self.repo.store_source(actor, source_uri, content, scope)?;
        let claim = Claim::new(ClaimId::new(), content, MemoryTier::Episodic);
        self.repo.store_claim(actor, &claim)?;
        let bridge = MempalaceGraphBridge::new(&self.repo);
        for source in self.repo.list_sources()? {
            if source.uri == source_uri {
                bridge.sync_source(&source)?;
            }
        }
        bridge.sync_claim(&claim)?;
        Ok(claim)
    }

    pub fn file_claim(&self, actor: &str, text: &str, tier: MemoryTier) -> Result<Claim> {
        let claim = Claim::new(ClaimId::new(), text, tier);
        self.repo.store_claim(actor, &claim)?;
        MempalaceGraphBridge::new(&self.repo).sync_claim(&claim)?;
        Ok(claim)
    }

    pub fn supersede(
        &self,
        actor: &str,
        previous_id: ClaimId,
        text: &str,
        confidence: f64,
        quality_score: f64,
    ) -> Result<ClaimReplacement> {
        let previous = self
            .repo
            .get_claim(previous_id)?
            .ok_or(KernelError::ClaimNotFound(previous_id))?;
        let replacement = previous.superseded_by(ClaimId::new(), text, confidence, quality_score);
        self.repo.store_claim_replacement(actor, &replacement)?;
        let bridge = MempalaceGraphBridge::new(&self.repo);
        bridge.sync_claim(&replacement.previous)?;
        bridge.sync_claim(&replacement.current)?;
        Ok(replacement)
    }

    pub fn query(&self, actor: &str, query: &str, options: QueryOptions) -> Result<QueryResult> {
        self.query_internal(actor, query, options, None::<&CosineVectorRetriever<'_, NoopEmbeddingProvider>>)
    }

    pub fn query_with_vector_retriever<P>(
        &self,
        actor: &str,
        query: &str,
        options: QueryOptions,
        retriever: &CosineVectorRetriever<'_, P>,
    ) -> Result<QueryResult>
    where
        P: EmbeddingProvider,
    {
        self.query_internal(actor, query, options, Some(retriever))
    }

    fn query_internal<P>(
        &self,
        actor: &str,
        query: &str,
        options: QueryOptions,
        vector_retriever: Option<&CosineVectorRetriever<'_, P>>,
    ) -> Result<QueryResult>
    where
        P: EmbeddingProvider,
    {
        let claims = self.repo.list_claims()?;
        let keyword_hits = SqliteFtsRetriever::new(&self.repo).retrieve(query)?;
        let vector_hits = if self.runtime_config.retrieval.vector.enabled {
            match vector_retriever {
                Some(retriever) => retriever.retrieve(query)?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let graph_hits = if self.runtime_config.retrieval.graph.enabled {
            MempalaceGraphRetriever::new(
                &self.repo,
                self.runtime_config.retrieval.graph.walk_depth,
                self.runtime_config.retrieval.graph.max_neighbors,
            )
            .retrieve(query)?
        } else {
            Vec::new()
        };
        let ranked = retrieve_claims(&claims, &keyword_hits, &vector_hits, &graph_hits, query);
        let top_claims: Vec<Claim> = ranked.into_iter().take(5).collect();

        self.repo.record_event(
            OutboxEventKind::QueryServed,
            query,
            serde_json::json!({ "query": query, "matches": top_claims.len() }),
        )?;
        self.repo
            .record_audit(actor, AuditAction::Query, &format!("Served query `{query}`"))?;

        let page_path = if options.write_page {
            let title = options
                .page_title
                .clone()
                .unwrap_or_else(|| format!("analysis-{}", slugify(query)));
            let slug = slugify(&title);
            let body = render_query_page(query, &top_claims);
            let path = write_page(&self.wiki_dir, &slug, &title, &body)?;
            self.repo
                .store_page(actor, &slug, &title, &path.to_string_lossy(), &body)?;
            let bridge = MempalaceGraphBridge::new(&self.repo);
            for page in self.repo.list_pages()? {
                if page.slug == slug {
                    bridge.sync_page(&page)?;
                }
            }
            Some(path)
        } else {
            None
        };

        Ok(QueryResult {
            claims: top_claims,
            page_path,
        })
    }

    pub fn run_lint(&self, actor: &str) -> Result<Vec<LintIssue>> {
        let claims = self.repo.list_claims()?;
        let issues = run_lint(&self.wiki_dir, &claims)?;
        let body = render_report(&issues);
        let report_path = write_report(&self.wiki_dir, "lint-latest", &body)?;

        self.repo.record_event(
            OutboxEventKind::LintRunFinished,
            "lint",
            serde_json::json!({
                "issues": issues.len(),
                "report": report_path.to_string_lossy(),
            }),
        )?;
        self.repo
            .record_audit(actor, AuditAction::Lint, &format!("Ran lint with {} issues", issues.len()))?;
        Ok(issues)
    }

    pub fn export_outbox(&self, consumer: &str) -> Result<Vec<OutboxEvent>> {
        Ok(self.repo.export_outbox(consumer)?)
    }

    pub fn ack_outbox(&self, consumer: &str, event_id: uuid::Uuid) -> Result<()> {
        self.repo.ack_outbox(consumer, event_id)?;
        Ok(())
    }

    pub fn wiki_dir(&self) -> &Path {
        &self.wiki_dir
    }

    pub fn repo(&self) -> &SqliteWikiRepository {
        &self.repo
    }

    pub fn rebuild_graph(&self) -> Result<()> {
        MempalaceGraphBridge::new(&self.repo).rebuild()?;
        Ok(())
    }

    pub fn rebuild_fts(&self) -> Result<usize> {
        Ok(self.repo.rebuild_fts()?)
    }

    pub fn sync_index<P>(&self, actor: &str, provider: Option<&P>) -> Result<SyncIndexResult>
    where
        P: EmbeddingProvider,
    {
        let mut embedded_claims = 0usize;
        if self.runtime_config.retrieval.vector.enabled {
            if let Some(provider) = provider {
                for claim in self
                    .repo
                    .list_claims_needing_embeddings(&self.runtime_config.retrieval.vector.model)?
                {
                    let content_hash = hash_text(&claim.text);
                    match provider.embed_text(&claim.text) {
                        Ok(vector) => {
                            self.repo.upsert_embedding(
                                claim.id,
                                &self.runtime_config.retrieval.vector.model,
                                &vector,
                                &content_hash,
                            )?;
                            embedded_claims += 1;
                        }
                        Err(error) => {
                            self.repo.mark_embedding_failed(
                                claim.id,
                                &self.runtime_config.retrieval.vector.model,
                                &content_hash,
                                &error.to_string(),
                            )?;
                        }
                    }
                }
            }
        }

        self.rebuild_graph()?;
        let (graph_nodes, graph_edges) = self.repo.graph_counts()?;
        self.repo.record_audit(
            actor,
            AuditAction::Crystallize,
            &format!(
                "Synced indexes: embeddings={}, graph_nodes={}, graph_edges={}",
                embedded_claims, graph_nodes, graph_edges
            ),
        )?;

        Ok(SyncIndexResult {
            embedded_claims,
            graph_nodes,
            graph_edges,
        })
    }

    pub fn provider_health(&self) -> Result<Vec<ProviderHealth>> {
        let (graph_nodes, graph_edges) = self.repo.graph_counts()?;
        let fts_count = self.repo.fts_count()?;
        let vector_model = &self.runtime_config.retrieval.vector.model;
        let pending_embeddings = self.repo.list_claims_needing_embeddings(vector_model)?.len();

        Ok(vec![
            ProviderHealth {
                provider_name: "keyword-fts5".to_owned(),
                status: if self.runtime_config.retrieval.keyword.enabled {
                    HealthStatus::Ready
                } else {
                    HealthStatus::Unavailable
                },
                message: format!("fts rows={fts_count}"),
                checked_at: Utc::now(),
            },
            ProviderHealth {
                provider_name: "vector-openai-compatible".to_owned(),
                status: if self.runtime_config.retrieval.vector.enabled
                    && !self.runtime_config.retrieval.vector.base_url.is_empty()
                {
                    if pending_embeddings > 0 {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Ready
                    }
                } else {
                    HealthStatus::Unavailable
                },
                message: format!(
                    "model={}, pending_embeddings={pending_embeddings}",
                    self.runtime_config.retrieval.vector.model
                ),
                checked_at: Utc::now(),
            },
            ProviderHealth {
                provider_name: "graph-mempalace-local".to_owned(),
                status: if self.runtime_config.retrieval.graph.enabled {
                    HealthStatus::Ready
                } else {
                    HealthStatus::Unavailable
                },
                message: format!("graph nodes={graph_nodes}, edges={graph_edges}"),
                checked_at: Utc::now(),
            },
        ])
    }
}

struct NoopEmbeddingProvider;

impl EmbeddingProvider for NoopEmbeddingProvider {
    fn embed_text(&self, _input: &str) -> crate::EmbeddingResult<Vec<f32>> {
        Ok(Vec::new())
    }
}

fn hash_text(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn render_query_page(query: &str, claims: &[Claim]) -> String {
    let mut body = format!("# Query: {query}\n\n## Results\n\n");
    if claims.is_empty() {
        body.push_str("No claims matched.\n");
    } else {
        for claim in claims {
            body.push_str(&format!(
                "- {} (`{}`)\n",
                claim.text, claim.id
            ));
        }
    }
    body
}

use std::path::{Path, PathBuf};

use thiserror::Error;
use wiki_core::{
    AuditAction, Claim, ClaimId, ClaimReplacement, LintIssue, MemoryTier, OutboxEvent,
    OutboxEventKind,
};
use wiki_storage::{SqliteWikiRepository, StorageError};

use crate::{
    lint::{render_report, run_lint},
    providers::keyword::{KeywordRetriever, SqliteFtsRetriever},
    retrieval::retrieve_claims,
    wiki::{ensure_layout, slugify, write_page, write_report},
};

pub type Result<T> = std::result::Result<T, KernelError>;

pub struct WikiEngine {
    repo: SqliteWikiRepository,
    wiki_dir: PathBuf,
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

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("claim not found: {0}")]
    ClaimNotFound(ClaimId),
}

impl WikiEngine {
    pub fn new(repo: SqliteWikiRepository, wiki_dir: impl AsRef<Path>) -> Result<Self> {
        let wiki_dir = wiki_dir.as_ref().to_path_buf();
        ensure_layout(&wiki_dir)?;
        Ok(Self { repo, wiki_dir })
    }

    pub fn ingest(&self, actor: &str, source_uri: &str, content: &str, scope: &str) -> Result<Claim> {
        self.repo.store_source(actor, source_uri, content, scope)?;
        let claim = Claim::new(ClaimId::new(), content, MemoryTier::Episodic);
        self.repo.store_claim(actor, &claim)?;
        Ok(claim)
    }

    pub fn file_claim(&self, actor: &str, text: &str, tier: MemoryTier) -> Result<Claim> {
        let claim = Claim::new(ClaimId::new(), text, tier);
        self.repo.store_claim(actor, &claim)?;
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
        Ok(replacement)
    }

    pub fn query(&self, actor: &str, query: &str, options: QueryOptions) -> Result<QueryResult> {
        let claims = self.repo.list_claims()?;
        let keyword_hits = SqliteFtsRetriever::new(&self.repo).retrieve(query)?;
        let ranked = retrieve_claims(&claims, &keyword_hits, query);
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

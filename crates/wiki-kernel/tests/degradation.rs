use tempfile::tempdir;
use wiki_core::{Claim, ClaimId, MemoryTier};
use wiki_kernel::{QueryOptions, RuntimeConfig, WikiEngine};
use wiki_storage::SqliteWikiRepository;

#[test]
fn query_records_provider_runs_and_degrades_when_vector_provider_is_unavailable() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let claim = Claim::new(
        ClaimId::new(),
        "Redis default TTL is 3600 seconds",
        MemoryTier::Semantic,
    );
    repo.store_claim("tester", &claim).expect("claim");

    let temp = tempdir().expect("tempdir");
    let engine = WikiEngine::with_config(
        repo,
        temp.path().join("wiki"),
        RuntimeConfig::vector_enabled_for_tests("embedding-small"),
    )
    .expect("engine");

    let result = engine
        .query("tester", "Redis TTL", QueryOptions::default())
        .expect("query should degrade, not fail");

    let provider_runs = engine.repo().list_provider_runs().expect("provider runs");
    let audit = engine.repo().list_audit_records().expect("audit");

    assert_eq!(result.claims[0].id, claim.id);
    assert!(provider_runs.iter().any(|run| {
        run.provider_name == "vector-openai-compatible" && run.status == "degraded"
    }));
    assert!(audit
        .iter()
        .any(|record| record.summary.contains("degraded providers")));
}

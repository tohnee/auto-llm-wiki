use tempfile::tempdir;
use wiki_core::{Claim, ClaimId, MemoryTier};
use wiki_kernel::{KeywordRetriever, SqliteFtsRetriever, WikiEngine};
use wiki_storage::SqliteWikiRepository;

#[test]
fn fts_keyword_retriever_returns_ranked_claims() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let strong = Claim::new(
        ClaimId::new(),
        "Redis TTL policy defaults to 3600 seconds",
        MemoryTier::Semantic,
    );
    let weak = Claim::new(
        ClaimId::new(),
        "Redis is used as a cache for session data",
        MemoryTier::Semantic,
    );

    repo.store_claim("tester", &strong).expect("store strong");
    repo.store_claim("tester", &weak).expect("store weak");

    let retriever = SqliteFtsRetriever::new(&repo);
    let hits = retriever.retrieve("Redis TTL").expect("fts hits");

    assert_eq!(hits[0].claim_id, strong.id);
    assert!(hits.iter().all(|hit| hit.rank >= 1));
}

#[test]
fn engine_query_uses_fts_keyword_results() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let strong = Claim::new(
        ClaimId::new(),
        "Redis TTL policy defaults to 3600 seconds",
        MemoryTier::Semantic,
    );
    let weak = Claim::new(
        ClaimId::new(),
        "Redis cache warmup notes",
        MemoryTier::Semantic,
    );

    repo.store_claim("tester", &strong).expect("store strong");
    repo.store_claim("tester", &weak).expect("store weak");

    let temp = tempdir().expect("tempdir");
    let engine = WikiEngine::new(repo, temp.path().join("wiki")).expect("engine");
    let result = engine
        .query("tester", "Redis TTL", Default::default())
        .expect("query");

    assert_eq!(result.claims[0].id, strong.id);
}

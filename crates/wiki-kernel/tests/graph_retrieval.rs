use wiki_core::{Claim, ClaimId, MemoryTier};
use wiki_kernel::MempalaceGraphRetriever;
use wiki_mempalace_bridge::MempalaceGraphBridge;
use wiki_storage::SqliteWikiRepository;

#[test]
fn graph_retrieval_returns_claims_connected_to_query_concepts() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let claim = Claim::new(
        ClaimId::new(),
        "Redis default TTL is 3600 seconds",
        MemoryTier::Semantic,
    );

    repo.store_source(
        "tester",
        "file:///notes/redis.md",
        "Redis throughput tuning and cache design notes",
        "private:me",
    )
    .expect("source");
    repo.store_claim("tester", &claim).expect("claim");
    repo.store_page(
        "tester",
        "redis-ops",
        "redis-ops",
        "wiki/pages/redis-ops.md",
        "# Redis Ops\n\nRedis throughput and cache warmup guide.\n",
    )
    .expect("page");

    let bridge = MempalaceGraphBridge::new(&repo);
    bridge.rebuild().expect("rebuild graph");

    let retriever = MempalaceGraphRetriever::new(&repo, 2, 32);
    let hits = retriever.retrieve("throughput").expect("graph hits");

    assert_eq!(hits[0].claim_id, claim.id);
}

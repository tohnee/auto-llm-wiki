use wiki_core::{Claim, ClaimId, MemoryTier};
use wiki_storage::SqliteWikiRepository;

#[test]
fn storing_claim_updates_fts_and_marks_embedding_pending() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let claim = Claim::new(
        ClaimId::new(),
        "Redis default TTL is 3600 seconds",
        MemoryTier::Semantic,
    );

    repo.store_claim("tester", &claim).expect("store claim");

    let matches = repo.search_fts_claim_ids("Redis TTL").expect("fts search");
    let embedding_state = repo
        .get_embedding_state(claim.id)
        .expect("embedding state")
        .expect("embedding row");

    assert_eq!(matches, vec![claim.id]);
    assert_eq!(embedding_state.status, "pending");
}

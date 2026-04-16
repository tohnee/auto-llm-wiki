use wiki_core::{Claim, ClaimId, MemoryTier, OutboxEventKind};
use wiki_storage::SqliteWikiRepository;

#[test]
fn storing_claim_emits_outbox_and_audit_records() {
    let repo = SqliteWikiRepository::open_in_memory().expect("in-memory db");
    let claim = Claim::new(
        ClaimId::new(),
        "Redis default TTL is 3600 seconds",
        MemoryTier::Semantic,
    );

    repo.store_claim("tester", &claim).expect("claim stored");

    let stored = repo.get_claim(claim.id).expect("claim lookup");
    let outbox = repo.list_outbox().expect("outbox");
    let audit = repo.list_audit_records().expect("audit");

    assert_eq!(stored.expect("claim exists").text, claim.text);
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].kind, OutboxEventKind::ClaimUpserted);
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].actor, "tester");
}

use chrono::{Duration, Utc};
use wiki_core::{
    fuse_ranked_results, retention_strength, Claim, ClaimId, MemoryTier, RankedClaim,
};

#[test]
fn superseding_claim_marks_old_claim_stale() {
    let old_id = ClaimId::new();
    let old_claim = Claim::new(
        old_id,
        "Redis default TTL is 3600 seconds",
        MemoryTier::Semantic,
    );

    let replacement = old_claim.superseded_by(
        ClaimId::new(),
        "Redis default TTL depends on key configuration",
        0.91,
        0.88,
    );

    assert!(replacement.previous.stale);
    assert_eq!(replacement.previous.id, old_id);
    assert_eq!(replacement.current.supersedes, Some(old_id));
    assert_eq!(replacement.current.tier, MemoryTier::Semantic);
}

#[test]
fn retention_strength_prefers_recent_and_frequently_accessed_claims() {
    let mut claim = Claim::new(
        ClaimId::new(),
        "Redis is used as a cache",
        MemoryTier::Semantic,
    );
    claim.access_count = 5;
    claim.updated_at = Utc::now() - Duration::hours(1);

    let score = retention_strength(&claim, Utc::now());

    assert!(score > 1.0);
}

#[test]
fn reciprocal_rank_fusion_combines_retrieval_paths() {
    let a = ClaimId::new();
    let b = ClaimId::new();

    let fused = fuse_ranked_results(
        &[
            RankedClaim::new(a, 1),
            RankedClaim::new(b, 2),
        ],
        &[
            RankedClaim::new(b, 1),
            RankedClaim::new(a, 3),
        ],
        &[
            RankedClaim::new(a, 2),
        ],
    );

    assert_eq!(fused[0].claim_id, a);
    assert!(fused[0].score > fused[1].score);
}

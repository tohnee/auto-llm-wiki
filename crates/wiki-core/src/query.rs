use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::claim::{Claim, ClaimId};

const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedClaim {
    pub claim_id: ClaimId,
    pub rank: usize,
}

impl RankedClaim {
    pub fn new(claim_id: ClaimId, rank: usize) -> Self {
        Self { claim_id, rank }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RankedResult {
    pub claim_id: ClaimId,
    pub score: f64,
}

pub fn fuse_ranked_results(
    bm25: &[RankedClaim],
    vector: &[RankedClaim],
    graph: &[RankedClaim],
) -> Vec<RankedResult> {
    let mut scores = std::collections::HashMap::<ClaimId, f64>::new();

    for path in [bm25, vector, graph] {
        for result in path {
            let contribution = 1.0 / (RRF_K + result.rank as f64);
            *scores.entry(result.claim_id).or_insert(0.0) += contribution;
        }
    }

    let mut fused: Vec<_> = scores
        .into_iter()
        .map(|(claim_id, score)| RankedResult { claim_id, score })
        .collect();
    fused.sort_by(|left, right| right.score.total_cmp(&left.score));
    fused
}

pub fn retention_strength(claim: &Claim, now: DateTime<Utc>) -> f64 {
    let age_hours = (now - claim.updated_at).num_minutes().max(0) as f64 / 60.0;
    let recency_factor = (24.0 / (24.0 + age_hours)).max(0.2);
    let access_factor = 1.0 + (claim.access_count as f64 * 0.15);
    let confidence_factor = (claim.confidence + claim.quality_score) / 2.0;

    recency_factor * access_factor * confidence_factor.max(0.2)
}

use std::collections::HashMap;

use chrono::Utc;
use wiki_core::{fuse_ranked_results, retention_strength, Claim, ClaimId, RankedClaim};

pub fn retrieve_claims(claims: &[Claim], keyword_hits: &[RankedClaim], query: &str) -> Vec<Claim> {
    let bm25 = keyword_hits.to_vec();
    let vector = rank_by_overlap(claims, query);
    let graph = rank_by_graph_hint(claims, query);

    let fused = fuse_ranked_results(&bm25, &vector, &graph);
    let claim_map: HashMap<ClaimId, Claim> = claims.iter().cloned().map(|claim| (claim.id, claim)).collect();

    let mut ranked: Vec<(f64, Claim)> = fused
        .into_iter()
        .filter_map(|result| {
            claim_map.get(&result.claim_id).cloned().map(|claim| {
                let score = result.score * retention_strength(&claim, Utc::now());
                (score, claim)
            })
        })
        .collect();

    if ranked.is_empty() && !claims.is_empty() {
        ranked = claims
            .iter()
            .cloned()
            .map(|claim| (retention_strength(&claim, Utc::now()), claim))
            .collect();
    }

    ranked.sort_by(|left, right| right.0.total_cmp(&left.0));
    ranked.into_iter().map(|(_, claim)| claim).collect()
}

fn rank_by_overlap(claims: &[Claim], query: &str) -> Vec<RankedClaim> {
    let query_tokens = tokenize(query);
    rank_claims(claims, |claim| {
        let claim_tokens = tokenize(&claim.text);
        let overlap = claim_tokens
            .iter()
            .filter(|token| query_tokens.contains(token))
            .count() as f64;
        overlap / (claim_tokens.len().max(1) as f64)
    })
}

fn rank_by_graph_hint(claims: &[Claim], query: &str) -> Vec<RankedClaim> {
    let tokens = tokenize(query);
    rank_claims(claims, |claim| {
        let lower = claim.text.to_lowercase();
        let first_match = tokens
            .iter()
            .enumerate()
            .find_map(|(index, token)| lower.contains(token.as_str()).then_some(index as f64 + 1.0))
            .unwrap_or(0.0);
        if first_match == 0.0 {
            0.0
        } else {
            1.0 / first_match
        }
    })
}

fn rank_claims<F>(claims: &[Claim], scorer: F) -> Vec<RankedClaim>
where
    F: Fn(&Claim) -> f64,
{
    let mut scored: Vec<(f64, ClaimId)> = claims
        .iter()
        .map(|claim| (scorer(claim), claim.id))
        .filter(|(score, _)| *score > 0.0)
        .collect();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));
    scored
        .into_iter()
        .enumerate()
        .map(|(index, (_, claim_id))| RankedClaim::new(claim_id, index + 1))
        .collect()
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

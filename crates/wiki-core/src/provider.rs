use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::claim::ClaimId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderHit {
    pub claim_id: ClaimId,
    pub raw_score: f64,
    pub provider_name: String,
    pub latency_ms: u64,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider_name: String,
    pub status: HealthStatus,
    pub message: String,
    pub checked_at: DateTime<Utc>,
}

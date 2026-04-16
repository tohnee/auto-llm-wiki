use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClaimId(Uuid);

impl ClaimId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

impl fmt::Display for ClaimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryTier {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub text: String,
    pub tier: MemoryTier,
    pub confidence: f64,
    pub quality_score: f64,
    pub supersedes: Option<ClaimId>,
    pub stale: bool,
    pub access_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Claim {
    pub fn new(id: ClaimId, text: impl Into<String>, tier: MemoryTier) -> Self {
        let now = Utc::now();
        Self {
            id,
            text: text.into(),
            tier,
            confidence: 1.0,
            quality_score: 1.0,
            supersedes: None,
            stale: false,
            access_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn superseded_by(
        &self,
        id: ClaimId,
        text: impl Into<String>,
        confidence: f64,
        quality_score: f64,
    ) -> ClaimReplacement {
        let now = Utc::now();
        let mut previous = self.clone();
        previous.stale = true;
        previous.updated_at = now;

        let mut current = Claim::new(id, text, self.tier);
        current.confidence = confidence;
        current.quality_score = quality_score;
        current.supersedes = Some(self.id);
        current.updated_at = now;

        ClaimReplacement { previous, current }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimReplacement {
    pub previous: Claim,
    pub current: Claim,
}

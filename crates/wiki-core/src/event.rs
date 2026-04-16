use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxEventKind {
    SourceIngested,
    ClaimUpserted,
    ClaimSuperseded,
    PageWritten,
    QueryServed,
    LintRunFinished,
    SessionCrystallized,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub kind: OutboxEventKind,
    pub aggregate_id: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Ingest,
    ClaimUpsert,
    ClaimSupersede,
    Query,
    Lint,
    Crystallize,
    PageWrite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: Uuid,
    pub actor: String,
    pub action: AuditAction,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}

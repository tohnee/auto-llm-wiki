use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use std::cell::RefCell;
use uuid::Uuid;
use wiki_core::{
    AuditAction, AuditRecord, Claim, ClaimId, MemoryTier, OutboxEvent, OutboxEventKind,
};

use crate::{repository::Result, schema::INIT_SQL};

pub struct SqliteWikiRepository {
    conn: RefCell<Connection>,
}

impl SqliteWikiRepository {
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(INIT_SQL)?;
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }

    pub fn store_claim(&self, actor: &str, claim: &Claim) -> Result<()> {
        let conn = self.conn.borrow_mut();
        conn.execute(
            "INSERT OR REPLACE INTO claims
             (id, text, tier, confidence, quality_score, supersedes, stale, access_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                claim.id.to_string(),
                claim.text,
                encode_tier(claim.tier),
                claim.confidence,
                claim.quality_score,
                claim.supersedes.map(|id| id.to_string()),
                claim.stale as i64,
                claim.access_count as i64,
                claim.created_at.to_rfc3339(),
                claim.updated_at.to_rfc3339(),
            ],
        )?;

        let event = OutboxEvent {
            id: Uuid::new_v4(),
            kind: OutboxEventKind::ClaimUpserted,
            aggregate_id: claim.id.to_string(),
            payload: json!({
                "claim_id": claim.id.to_string(),
                "text": claim.text,
            }),
            created_at: Utc::now(),
        };
        conn.execute(
            "INSERT INTO wiki_outbox (id, kind, aggregate_id, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.id.to_string(),
                encode_outbox_kind(&event.kind),
                event.aggregate_id,
                serde_json::to_string(&event.payload)?,
                event.created_at.to_rfc3339(),
            ],
        )?;

        let audit = AuditRecord {
            id: Uuid::new_v4(),
            actor: actor.to_owned(),
            action: AuditAction::ClaimUpsert,
            summary: format!("Stored claim {}", claim.id),
            created_at: Utc::now(),
        };
        conn.execute(
            "INSERT INTO audit_records (id, actor, action, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                audit.id.to_string(),
                audit.actor,
                encode_audit_action(&audit.action),
                audit.summary,
                audit.created_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn get_claim(&self, claim_id: ClaimId) -> Result<Option<Claim>> {
        let conn = self.conn.borrow();
        let claim = conn
            .query_row(
                "SELECT id, text, tier, confidence, quality_score, supersedes, stale, access_count, created_at, updated_at
                 FROM claims WHERE id = ?1",
                params![claim_id.to_string()],
                |row| {
                    Ok(Claim {
                        id: ClaimId::parse(&row.get::<_, String>(0)?).map_err(to_sql_err)?,
                        text: row.get(1)?,
                        tier: decode_tier(&row.get::<_, String>(2)?),
                        confidence: row.get(3)?,
                        quality_score: row.get(4)?,
                        supersedes: row
                            .get::<_, Option<String>>(5)?
                            .map(|raw| ClaimId::parse(&raw).map_err(to_sql_err))
                            .transpose()?,
                        stale: row.get::<_, i64>(6)? != 0,
                        access_count: row.get::<_, i64>(7)? as u32,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(to_sql_err)?,
                        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(to_sql_err)?,
                    })
                },
            )
            .optional()?;
        Ok(claim)
    }

    pub fn list_outbox(&self) -> Result<Vec<OutboxEvent>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, kind, aggregate_id, payload, created_at FROM wiki_outbox ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let payload = row.get::<_, String>(3)?;
            Ok(OutboxEvent {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(to_sql_err)?,
                kind: decode_outbox_kind(&row.get::<_, String>(1)?),
                aggregate_id: row.get(2)?,
                payload: serde_json::from_str(&payload).map_err(to_sql_err)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(to_sql_err)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn list_audit_records(&self) -> Result<Vec<AuditRecord>> {
        let conn = self.conn.borrow();
        let mut stmt = conn
            .prepare("SELECT id, actor, action, summary, created_at FROM audit_records ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(AuditRecord {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(to_sql_err)?,
                actor: row.get(1)?,
                action: decode_audit_action(&row.get::<_, String>(2)?),
                summary: row.get(3)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(to_sql_err)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

fn encode_tier(tier: MemoryTier) -> &'static str {
    match tier {
        MemoryTier::Working => "working",
        MemoryTier::Episodic => "episodic",
        MemoryTier::Semantic => "semantic",
        MemoryTier::Procedural => "procedural",
    }
}

fn decode_tier(raw: &str) -> MemoryTier {
    match raw {
        "working" => MemoryTier::Working,
        "episodic" => MemoryTier::Episodic,
        "procedural" => MemoryTier::Procedural,
        _ => MemoryTier::Semantic,
    }
}

fn encode_outbox_kind(kind: &OutboxEventKind) -> &'static str {
    match kind {
        OutboxEventKind::SourceIngested => "source_ingested",
        OutboxEventKind::ClaimUpserted => "claim_upserted",
        OutboxEventKind::ClaimSuperseded => "claim_superseded",
        OutboxEventKind::PageWritten => "page_written",
        OutboxEventKind::QueryServed => "query_served",
        OutboxEventKind::LintRunFinished => "lint_run_finished",
        OutboxEventKind::SessionCrystallized => "session_crystallized",
    }
}

fn decode_outbox_kind(raw: &str) -> OutboxEventKind {
    match raw {
        "source_ingested" => OutboxEventKind::SourceIngested,
        "claim_superseded" => OutboxEventKind::ClaimSuperseded,
        "page_written" => OutboxEventKind::PageWritten,
        "query_served" => OutboxEventKind::QueryServed,
        "lint_run_finished" => OutboxEventKind::LintRunFinished,
        "session_crystallized" => OutboxEventKind::SessionCrystallized,
        _ => OutboxEventKind::ClaimUpserted,
    }
}

fn encode_audit_action(action: &AuditAction) -> &'static str {
    match action {
        AuditAction::Ingest => "ingest",
        AuditAction::ClaimUpsert => "claim_upsert",
        AuditAction::ClaimSupersede => "claim_supersede",
        AuditAction::Query => "query",
        AuditAction::Lint => "lint",
        AuditAction::Crystallize => "crystallize",
        AuditAction::PageWrite => "page_write",
    }
}

fn decode_audit_action(raw: &str) -> AuditAction {
    match raw {
        "ingest" => AuditAction::Ingest,
        "claim_supersede" => AuditAction::ClaimSupersede,
        "query" => AuditAction::Query,
        "lint" => AuditAction::Lint,
        "crystallize" => AuditAction::Crystallize,
        "page_write" => AuditAction::PageWrite,
        _ => AuditAction::ClaimUpsert,
    }
}

fn to_sql_err<E>(err: E) -> rusqlite::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(err),
    )
}

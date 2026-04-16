use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::{cell::RefCell, path::Path};
use uuid::Uuid;
use wiki_core::{
    AuditAction, AuditRecord, Claim, ClaimId, ClaimReplacement, MemoryTier, OutboxEvent,
    OutboxEventKind,
};

use crate::{repository::Result, schema::INIT_SQL};

pub struct SqliteWikiRepository {
    conn: RefCell<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingState {
    pub claim_id: ClaimId,
    pub status: String,
}

impl SqliteWikiRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(INIT_SQL)?;
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(INIT_SQL)?;
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }

    pub fn store_source(&self, actor: &str, uri: &str, content: &str, scope: &str) -> Result<()> {
        let now = Utc::now();
        let source_id = Uuid::new_v4().to_string();
        {
            let conn = self.conn.borrow_mut();
            conn.execute(
                "INSERT INTO sources (id, uri, content, scope, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![source_id, uri, content, scope, now.to_rfc3339()],
            )?;
        }
        self.record_event(
            OutboxEventKind::SourceIngested,
            uri,
            serde_json::json!({ "uri": uri, "scope": scope }),
        )?;
        self.record_audit(actor, AuditAction::Ingest, &format!("Ingested source {uri}"))?;
        Ok(())
    }

    pub fn store_page(&self, actor: &str, slug: &str, title: &str, path: &str, body: &str) -> Result<()> {
        let now = Utc::now();
        {
            let conn = self.conn.borrow_mut();
            conn.execute(
                "INSERT OR REPLACE INTO pages (slug, title, path, body, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, COALESCE((SELECT created_at FROM pages WHERE slug = ?1), ?5), ?5)",
                params![slug, title, path, body, now.to_rfc3339()],
            )?;
        }
        self.record_event(
            OutboxEventKind::PageWritten,
            slug,
            serde_json::json!({ "slug": slug, "title": title, "path": path }),
        )?;
        self.record_audit(actor, AuditAction::PageWrite, &format!("Wrote page {slug}"))?;
        Ok(())
    }

    pub fn store_claim(&self, actor: &str, claim: &Claim) -> Result<()> {
        {
            let conn = self.conn.borrow_mut();
            write_claim(&conn, claim)?;
            conn.execute(
                "INSERT INTO claim_fts (claim_id, text) VALUES (?1, ?2)",
                params![claim.id.to_string(), claim.text],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO claim_embeddings
                 (claim_id, model, dim, vector_json, content_hash, status, last_error, embedded_at, updated_at)
                 VALUES (?1, NULL, NULL, NULL, NULL, 'pending', NULL, NULL, ?2)",
                params![claim.id.to_string(), Utc::now().to_rfc3339()],
            )?;
        }

        self.record_event(
            OutboxEventKind::ClaimUpserted,
            &claim.id.to_string(),
            serde_json::json!({
                "claim_id": claim.id.to_string(),
                "text": claim.text,
            }),
        )?;
        self.record_audit(actor, AuditAction::ClaimUpsert, &format!("Stored claim {}", claim.id))?;
        Ok(())
    }

    pub fn store_claim_replacement(&self, actor: &str, replacement: &ClaimReplacement) -> Result<()> {
        {
            let conn = self.conn.borrow_mut();
            write_claim(&conn, &replacement.previous)?;
            write_claim(&conn, &replacement.current)?;
        }
        self.record_event(
            OutboxEventKind::ClaimSuperseded,
            &replacement.current.id.to_string(),
            serde_json::json!({
                "claim_id": replacement.current.id.to_string(),
                "supersedes": replacement.previous.id.to_string(),
            }),
        )?;
        self.record_audit(
            actor,
            AuditAction::ClaimSupersede,
            &format!(
                "Superseded claim {} with {}",
                replacement.previous.id, replacement.current.id
            ),
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
                map_claim,
            )
            .optional()?;
        Ok(claim)
    }

    pub fn list_claims(&self) -> Result<Vec<Claim>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, text, tier, confidence, quality_score, supersedes, stale, access_count, created_at, updated_at
             FROM claims ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], map_claim)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn list_outbox(&self) -> Result<Vec<OutboxEvent>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, kind, aggregate_id, payload, created_at FROM wiki_outbox ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], map_outbox_event)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn search_fts_claim_ids(&self, query: &str) -> Result<Vec<ClaimId>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT claim_id FROM claim_fts WHERE claim_fts MATCH ?1 ORDER BY bm25(claim_fts)",
        )?;
        let rows = stmt.query_map(params![query], |row| {
            ClaimId::parse(&row.get::<_, String>(0)?).map_err(to_sql_err)
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_embedding_state(&self, claim_id: ClaimId) -> Result<Option<EmbeddingState>> {
        let conn = self.conn.borrow();
        let state = conn
            .query_row(
                "SELECT claim_id, status FROM claim_embeddings WHERE claim_id = ?1",
                params![claim_id.to_string()],
                |row| {
                    Ok(EmbeddingState {
                        claim_id: ClaimId::parse(&row.get::<_, String>(0)?).map_err(to_sql_err)?,
                        status: row.get(1)?,
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    pub fn export_outbox(&self, consumer: &str) -> Result<Vec<OutboxEvent>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT o.id, o.kind, o.aggregate_id, o.payload, o.created_at
             FROM wiki_outbox o
             LEFT JOIN outbox_consumers c
               ON c.event_id = o.id AND c.consumer = ?1
             WHERE c.event_id IS NULL
             ORDER BY o.created_at ASC",
        )?;
        let rows = stmt.query_map(params![consumer], map_outbox_event)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn ack_outbox(&self, consumer: &str, event_id: Uuid) -> Result<()> {
        let conn = self.conn.borrow_mut();
        conn.execute(
            "INSERT OR REPLACE INTO outbox_consumers (consumer, event_id, acked_at) VALUES (?1, ?2, ?3)",
            params![consumer, event_id.to_string(), Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn record_event(&self, kind: OutboxEventKind, aggregate_id: &str, payload: Value) -> Result<()> {
        let event = OutboxEvent {
            id: Uuid::new_v4(),
            kind,
            aggregate_id: aggregate_id.to_owned(),
            payload,
            created_at: Utc::now(),
        };
        let conn = self.conn.borrow_mut();
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
        Ok(())
    }

    pub fn record_audit(&self, actor: &str, action: AuditAction, summary: &str) -> Result<()> {
        let audit = AuditRecord {
            id: Uuid::new_v4(),
            actor: actor.to_owned(),
            action,
            summary: summary.to_owned(),
            created_at: Utc::now(),
        };
        let conn = self.conn.borrow_mut();
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
                created_at: parse_time(&row.get::<_, String>(4)?)?,
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

fn write_claim(conn: &Connection, claim: &Claim) -> Result<()> {
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
    Ok(())
}

fn map_claim(row: &rusqlite::Row<'_>) -> rusqlite::Result<Claim> {
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
        created_at: parse_time(&row.get::<_, String>(8)?)?,
        updated_at: parse_time(&row.get::<_, String>(9)?)?,
    })
}

fn map_outbox_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxEvent> {
    let payload = row.get::<_, String>(3)?;
    Ok(OutboxEvent {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(to_sql_err)?,
        kind: decode_outbox_kind(&row.get::<_, String>(1)?),
        aggregate_id: row.get(2)?,
        payload: serde_json::from_str(&payload).map_err(to_sql_err)?,
        created_at: parse_time(&row.get::<_, String>(4)?)?,
    })
}

fn parse_time(raw: &str) -> rusqlite::Result<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(to_sql_err)
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

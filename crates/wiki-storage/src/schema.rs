pub const INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS claims (
    id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    tier TEXT NOT NULL,
    confidence REAL NOT NULL,
    quality_score REAL NOT NULL,
    supersedes TEXT,
    stale INTEGER NOT NULL,
    access_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    uri TEXT NOT NULL,
    content TEXT NOT NULL,
    scope TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pages (
    slug TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    path TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS claim_fts USING fts5(
    claim_id UNINDEXED,
    text,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS claim_embeddings (
    claim_id TEXT PRIMARY KEY,
    model TEXT,
    dim INTEGER,
    vector_json TEXT,
    content_hash TEXT,
    status TEXT NOT NULL,
    last_error TEXT,
    embedded_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graph_nodes (
    node_id TEXT PRIMARY KEY,
    node_type TEXT NOT NULL,
    external_ref TEXT NOT NULL,
    label TEXT NOT NULL,
    payload_json TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graph_edges (
    edge_id TEXT PRIMARY KEY,
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    weight REAL NOT NULL,
    payload_json TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_runs (
    run_id TEXT PRIMARY KEY,
    provider_name TEXT NOT NULL,
    operation TEXT NOT NULL,
    target_ref TEXT NOT NULL,
    status TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    error_message TEXT,
    payload_json TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS wiki_outbox (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS outbox_consumers (
    consumer TEXT NOT NULL,
    event_id TEXT NOT NULL,
    acked_at TEXT NOT NULL,
    PRIMARY KEY (consumer, event_id)
);

CREATE TABLE IF NOT EXISTS audit_records (
    id TEXT PRIMARY KEY,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    summary TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;

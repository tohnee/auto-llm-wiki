# Auto LLM Wiki

`auto-llm-wiki` is a Rust workspace that recreates the core ideas behind an LLM-maintained wiki:

- raw sources are ingested once
- claims are persisted with lifecycle metadata
- retrieval uses three channels fused with RRF
- state changes emit outbox and audit records
- markdown wiki pages are projected to a local `wiki/` directory

## Workspace

- `crates/wiki-core`: domain models, claim lifecycle, ranking helpers, lint issue types
- `crates/wiki-storage`: SQLite persistence, outbox, audit, sources and page metadata
- `crates/wiki-kernel`: ingest, query, lint, wiki projection workflows
- `crates/wiki-mempalace-bridge`: placeholder crate for external graph bridge integration
- `crates/wiki-cli`: command line interface

## Current Capability

- claim lifecycle with `supersedes`, `stale`, `confidence`, `quality_score`, `access_count`
- source ingest and claim persistence
- production-oriented retrieval building blocks:
  - SQLite FTS5 keyword retrieval
  - OpenAI-compatible embeddings provider
  - local cosine similarity vector retrieval
  - local mempalace graph bridge and graph walk retrieval
- markdown projection:
  - `index.md`
  - `log.md`
  - `pages/`
  - `concepts/`
  - `reports/`
- lint checks:
  - broken wikilinks
  - orphan pages
  - stale claims not referenced in pages
  - missing cross references
- outbox export and per-consumer ack
- audit trail for ingest, query, lint, page write, claim upsert, supersede

## Quick Start

```bash
cargo test
```

Create `wiki-config.toml`:

```toml
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "https://api.example.com/v1"
api_key = "env:EMBEDDING_API_KEY"
model = "embedding-small"
timeout_ms = 30000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  ingest "file:///notes/redis.md" "Redis default TTL is 3600 seconds" \
  --scope private:me
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  sync-index
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  query "Redis TTL" \
  --write-page \
  --page-title analysis-redis
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  lint
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  provider-health
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  rebuild-fts
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  rebuild-graph
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  outbox export --consumer demo
```

```bash
cargo run -p wiki-cli -- llm-smoke --db wiki.db --wiki-dir wiki --prompt "Say 'ok' only."
```

## Commands

- `ingest <source> <content> [--scope]`
- `file-claim <text> [--tier]`
- `supersede <claim_id> <text> [--confidence] [--quality-score]`
- `query <query> [--write-page] [--page-title]`
- `lint`
- `sync-index`
- `rebuild-fts`
- `rebuild-graph`
- `provider-health`
- `outbox export [--consumer]`
- `outbox ack <event_id> [--consumer]`
- `llm-smoke [--config] [--prompt]`

## Verification

```bash
cargo test
```

```bash
bash scripts/e2e.sh
```

## Notes

- Retrieval is intentionally implemented with replaceable heuristic channels for now; real BM25, vector search and graph walk providers can be plugged in later.
- `wiki-mempalace-bridge` is a placeholder integration surface and is not wired to an external graph service yet.

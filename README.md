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
- naive three-path retrieval:
  - keyword rank
  - token overlap rank
  - graph-hint rank
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

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  ingest "file:///notes/redis.md" "Redis default TTL is 3600 seconds" \
  --scope private:me
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  query "Redis TTL" \
  --write-page \
  --page-title analysis-redis
```

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  lint
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

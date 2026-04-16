#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
DB_PATH="$TMP_DIR/wiki.db"
WIKI_DIR="$TMP_DIR/wiki"
EVENTS_JSON="$TMP_DIR/events.json"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cd "$ROOT_DIR"

echo "[e2e] ingest"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  ingest "file:///notes/redis.md" "Redis default TTL is 3600 seconds" \
  --scope private:me >/dev/null

echo "[e2e] file-claim"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  file-claim "Redis is used as a cache" \
  --tier semantic >/dev/null

echo "[e2e] query"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  query "Redis TTL" \
  --write-page \
  --page-title analysis-redis >/dev/null

echo "[e2e] lint"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  lint >/dev/null

echo "[e2e] outbox export"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  outbox export --consumer e2e >"$EVENTS_JSON"

python3 - "$DB_PATH" "$WIKI_DIR" "$EVENTS_JSON" <<'PY'
import json
import subprocess
import sys

db_path, wiki_dir, events_json = sys.argv[1:]
with open(events_json, "r", encoding="utf-8") as fh:
    events = json.load(fh)

assert events, "expected at least one outbox event"

for event in events:
    subprocess.run(
        [
            "cargo",
            "run",
            "-p",
            "wiki-cli",
            "--",
            "--db",
            db_path,
            "--wiki-dir",
            wiki_dir,
            "outbox",
            "ack",
            event["id"],
            "--consumer",
            "e2e",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )
PY

echo "[e2e] llm smoke"
SMOKE_OUTPUT="$(cargo run -p wiki-cli -- --db "$DB_PATH" --wiki-dir "$WIKI_DIR" llm-smoke --prompt "Say 'ok' only.")"
test "$SMOKE_OUTPUT" = "ok"

test -f "$WIKI_DIR/pages/analysis-redis.md"
test -f "$WIKI_DIR/reports/lint-latest.md"

echo "[e2e] ok"

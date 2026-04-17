#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
DB_PATH="$TMP_DIR/wiki.db"
WIKI_DIR="$TMP_DIR/wiki"
EVENTS_JSON="$TMP_DIR/events.json"
CONFIG_PATH="$TMP_DIR/wiki-config.toml"
EMBED_SERVER_PORT_FILE="$TMP_DIR/embed-port"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cd "$ROOT_DIR"

python3 - "$TMP_DIR" <<'PY' &
import json
import socket
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

tmp_dir = sys.argv[1]

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/embeddings":
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length", "0"))
        _ = self.rfile.read(length)
        body = json.dumps({"data": [{"embedding": [0.9, 0.1, 0.0]}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return

server = HTTPServer(("127.0.0.1", 0), Handler)
with open(f"{tmp_dir}/embed-port", "w", encoding="utf-8") as fh:
    fh.write(str(server.server_port))
for _ in range(3):
    server.handle_request()
server.server_close()
PY
SERVER_PID=$!

for _ in $(seq 1 50); do
  if [ -f "$EMBED_SERVER_PORT_FILE" ]; then
    break
  fi
  sleep 0.1
done

EMBED_PORT="$(cat "$EMBED_SERVER_PORT_FILE")"
cat >"$CONFIG_PATH" <<EOF
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "http://127.0.0.1:${EMBED_PORT}"
api_key = "e2e-local"
model = "embedding-small"
timeout_ms = 5000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
EOF

echo "[e2e] ingest"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  ingest "file:///notes/redis.md" "Redis default TTL is 3600 seconds" \
  --scope private:me >/dev/null

echo "[e2e] file-claim"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  file-claim "Redis is used as a cache" \
  --tier semantic >/dev/null

echo "[e2e] sync-index"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  sync-index >/dev/null

echo "[e2e] query"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  query "Redis TTL" \
  --write-page \
  --page-title analysis-redis >/dev/null

echo "[e2e] lint"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  lint >/dev/null

echo "[e2e] provider-health"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  provider-health >/dev/null

echo "[e2e] rebuild-fts"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  rebuild-fts >/dev/null

echo "[e2e] rebuild-graph"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
  rebuild-graph >/dev/null

echo "[e2e] outbox export"
cargo run -p wiki-cli -- \
  --db "$DB_PATH" \
  --wiki-dir "$WIKI_DIR" \
  --config "$CONFIG_PATH" \
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
SMOKE_OUTPUT="$(cargo run -p wiki-cli -- --db "$DB_PATH" --wiki-dir "$WIKI_DIR" --config "$CONFIG_PATH" llm-smoke --prompt "Say 'ok' only.")"
test "$SMOKE_OUTPUT" = "ok"

test -f "$WIKI_DIR/pages/analysis-redis.md"
test -f "$WIKI_DIR/reports/lint-latest.md"
wait "$SERVER_PID"

echo "[e2e] ok"

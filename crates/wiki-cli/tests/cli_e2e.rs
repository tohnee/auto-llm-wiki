use std::{fs, process::Command};

use serde_json::Value;
use tempfile::tempdir;

#[test]
fn ingest_query_lint_flow_succeeds() {
    let temp = tempdir().expect("temp dir");
    let db = temp.path().join("wiki.db");
    let wiki_dir = temp.path().join("wiki");
    let bin = env!("CARGO_BIN_EXE_wiki-cli");

    let ingest = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "ingest",
            "file:///notes/redis.md",
            "Redis default TTL is 3600 seconds",
            "--scope",
            "private:me",
        ])
        .output()
        .expect("ingest");
    assert!(ingest.status.success(), "{:?}", ingest);

    let query = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "query",
            "Redis TTL",
            "--write-page",
            "--page-title",
            "analysis-redis",
        ])
        .output()
        .expect("query");
    assert!(query.status.success(), "{:?}", query);

    let lint = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "lint",
        ])
        .output()
        .expect("lint");
    assert!(lint.status.success(), "{:?}", lint);

    assert!(wiki_dir.join("pages/analysis-redis.md").exists());
    assert!(wiki_dir.join("reports/lint-latest.md").exists());
    let page = fs::read_to_string(wiki_dir.join("pages/analysis-redis.md")).expect("page");
    assert!(page.contains("Redis default TTL is 3600 seconds"));
}

#[test]
fn outbox_export_and_ack_round_trip() {
    let temp = tempdir().expect("temp dir");
    let db = temp.path().join("wiki.db");
    let wiki_dir = temp.path().join("wiki");
    let bin = env!("CARGO_BIN_EXE_wiki-cli");

    let ingest = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "ingest",
            "file:///notes/redis.md",
            "Redis is used as a cache",
        ])
        .output()
        .expect("ingest");
    assert!(ingest.status.success(), "{:?}", ingest);

    let export = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "outbox",
            "export",
            "--consumer",
            "test-consumer",
        ])
        .output()
        .expect("export");
    assert!(export.status.success(), "{:?}", export);

    let events: Vec<Value> = serde_json::from_slice(&export.stdout).expect("json export");
    assert!(!events.is_empty());

    for event in &events {
        let event_id = event["id"].as_str().expect("event id");
        let ack = Command::new(bin)
            .args([
                "--db",
                db.to_str().unwrap(),
                "--wiki-dir",
                wiki_dir.to_str().unwrap(),
                "outbox",
                "ack",
                event_id,
                "--consumer",
                "test-consumer",
            ])
            .output()
            .expect("ack");
        assert!(ack.status.success(), "{:?}", ack);
    }

    let export_again = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "outbox",
            "export",
            "--consumer",
            "test-consumer",
        ])
        .output()
        .expect("second export");
    assert!(export_again.status.success(), "{:?}", export_again);
    let events_after: Vec<Value> =
        serde_json::from_slice(&export_again.stdout).expect("json export after ack");
    assert!(events_after.is_empty());
}

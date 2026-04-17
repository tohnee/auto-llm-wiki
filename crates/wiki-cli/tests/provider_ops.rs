use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
};

use serde_json::Value;
use tempfile::tempdir;
use wiki_storage::SqliteWikiRepository;

#[test]
fn sync_index_and_provider_health_commands_succeed() {
    let temp = tempdir().expect("temp dir");
    let db = temp.path().join("wiki.db");
    let wiki_dir = temp.path().join("wiki");
    let config = temp.path().join("wiki-config.toml");
    let bin = env!("CARGO_BIN_EXE_wiki-cli");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 8192];
        let read = stream.read(&mut buffer).expect("read");
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.contains("POST /embeddings HTTP/1.1"));
        let body = r#"{"data":[{"embedding":[0.9,0.1,0.0]}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    fs::write(
        &config,
        format!(
            r#"
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "http://{addr}"
api_key = "local-test"
model = "embedding-small"
timeout_ms = 5000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
"#
        ),
    )
    .expect("write config");

    let ingest = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "ingest",
            "file:///notes/redis.md",
            "Redis default TTL is 3600 seconds",
        ])
        .output()
        .expect("ingest");
    assert!(ingest.status.success(), "{:?}", ingest);

    let sync = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "sync-index",
        ])
        .output()
        .expect("sync-index");
    assert!(sync.status.success(), "{:?}", sync);

    let repo = SqliteWikiRepository::open(&db).expect("repo");
    let claims = repo.list_claims().expect("claims");
    let embedding = repo
        .get_embedding_state(claims[0].id)
        .expect("state")
        .expect("embedding row");
    assert_eq!(embedding.status, "ready");
    assert!(!repo.list_graph_nodes().expect("graph nodes").is_empty());

    let health = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "provider-health",
        ])
        .output()
        .expect("provider-health");
    assert!(health.status.success(), "{:?}", health);
    let health_json: Vec<Value> = serde_json::from_slice(&health.stdout).expect("health json");
    assert_eq!(health_json.len(), 3);

    let rebuild_fts = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "rebuild-fts",
        ])
        .output()
        .expect("rebuild fts");
    assert!(rebuild_fts.status.success(), "{:?}", rebuild_fts);

    let rebuild_graph = Command::new(bin)
        .args([
            "--db",
            db.to_str().unwrap(),
            "--wiki-dir",
            wiki_dir.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "rebuild-graph",
        ])
        .output()
        .expect("rebuild graph");
    assert!(rebuild_graph.status.success(), "{:?}", rebuild_graph);

    handle.join().expect("server thread");
}

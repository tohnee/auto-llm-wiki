use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use tempfile::tempdir;
use wiki_core::{Claim, ClaimId, MemoryTier};
use wiki_kernel::{
    CosineVectorRetriever, EmbeddingProvider, GraphConfig, KeywordConfig,
    OpenAiCompatibleEmbeddingClient, QueryOptions, RetrievalConfig, RuntimeConfig, VectorConfig,
    WikiEngine,
};
use wiki_storage::SqliteWikiRepository;

#[test]
fn openai_compatible_embedding_client_requests_and_parses_embeddings() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 4096];
        let read = stream.read(&mut buffer).expect("read");
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.contains("POST /embeddings HTTP/1.1"));
        assert!(request.contains("\"model\":\"embedding-small\""));

        let body = r#"{"data":[{"embedding":[0.9,0.1,0.0]}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    let client = OpenAiCompatibleEmbeddingClient::new(
        format!("http://{addr}"),
        Some("secret".to_owned()),
        "embedding-small".to_owned(),
        5_000,
    )
    .expect("client");

    let vector = client.embed_text("Redis TTL").expect("embedding");
    handle.join().expect("server thread");

    assert_eq!(vector, vec![0.9, 0.1, 0.0]);
}

#[test]
fn vector_retrieval_uses_cached_embeddings_and_cosine_similarity() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let strong = Claim::new(
        ClaimId::new(),
        "Redis TTL policy defaults to 3600 seconds",
        MemoryTier::Semantic,
    );
    let weak = Claim::new(
        ClaimId::new(),
        "Redis cache warmup notes",
        MemoryTier::Semantic,
    );

    repo.store_claim("tester", &strong).expect("store strong");
    repo.store_claim("tester", &weak).expect("store weak");
    repo.upsert_embedding(strong.id, "embedding-small", &[0.9, 0.1, 0.0], "hash-strong")
        .expect("strong embedding");
    repo.upsert_embedding(weak.id, "embedding-small", &[0.1, 0.9, 0.0], "hash-weak")
        .expect("weak embedding");

    let provider = StaticEmbeddingProvider(vec![0.95, 0.05, 0.0]);
    let retriever = CosineVectorRetriever::new(&repo, &provider, "embedding-small", 5);
    let hits = retriever.retrieve("Redis TTL").expect("hits");

    assert_eq!(hits[0].claim_id, strong.id);

    let repo_for_engine = SqliteWikiRepository::open_in_memory().expect("repo for engine");
    repo_for_engine
        .store_claim("tester", &strong)
        .expect("store strong again");
    repo_for_engine
        .store_claim("tester", &weak)
        .expect("store weak again");
    repo_for_engine
        .upsert_embedding(strong.id, "embedding-small", &[0.9, 0.1, 0.0], "hash-strong")
        .expect("strong embedding again");
    repo_for_engine
        .upsert_embedding(weak.id, "embedding-small", &[0.1, 0.9, 0.0], "hash-weak")
        .expect("weak embedding again");

    let temp = tempdir().expect("tempdir");
    let engine = WikiEngine::with_config(
        repo_for_engine,
        temp.path().join("wiki"),
        RuntimeConfig::vector_enabled_for_tests("embedding-small"),
    )
    .expect("engine");
    let engine_retriever = CosineVectorRetriever::new(&repo, &provider, "embedding-small", 5);
    let result = engine
        .query_with_vector_retriever(
            "tester",
            "Redis TTL",
            QueryOptions::default(),
            &engine_retriever,
        )
        .expect("query");

    assert_eq!(result.claims[0].id, strong.id);
}

#[test]
fn query_automatically_uses_real_vector_provider_from_runtime_config() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer).expect("read");
        let body = r#"{"data":[{"embedding":[1.0,0.0]}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let strong = Claim::new(
        ClaimId::new(),
        "Completely unrelated words here",
        MemoryTier::Semantic,
    );
    let weak = Claim::new(
        ClaimId::new(),
        "Another claim with different semantics",
        MemoryTier::Semantic,
    );
    repo.store_claim("tester", &strong).expect("store strong");
    repo.store_claim("tester", &weak).expect("store weak");
    repo.upsert_embedding(strong.id, "embedding-small", &[1.0, 0.0], "hash-strong")
        .expect("strong embedding");
    repo.upsert_embedding(weak.id, "embedding-small", &[0.0, 1.0], "hash-weak")
        .expect("weak embedding");

    let temp = tempdir().expect("tempdir");
    let engine = WikiEngine::with_config(
        repo,
        temp.path().join("wiki"),
        RuntimeConfig {
            retrieval: RetrievalConfig {
                keyword: KeywordConfig {
                    enabled: true,
                    top_k: 20,
                },
                vector: VectorConfig {
                    enabled: true,
                    base_url: format!("http://{addr}"),
                    api_key: Some("secret".to_owned()),
                    model: "embedding-small".to_owned(),
                    timeout_ms: 5_000,
                    batch_size: 16,
                    top_k: 20,
                },
                graph: GraphConfig {
                    enabled: false,
                    walk_depth: 2,
                    max_neighbors: 32,
                    top_k: 20,
                },
            },
        },
    )
    .expect("engine");

    let result = engine
        .query("tester", "vector-only-query", QueryOptions::default())
        .expect("query");
    handle.join().expect("server thread");

    assert_eq!(result.claims[0].id, strong.id);
}

struct StaticEmbeddingProvider(Vec<f32>);

impl EmbeddingProvider for StaticEmbeddingProvider {
    fn embed_text(&self, _input: &str) -> wiki_kernel::EmbeddingResult<Vec<f32>> {
        Ok(self.0.clone())
    }
}

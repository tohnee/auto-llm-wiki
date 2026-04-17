use std::{env, fs};

use tempfile::tempdir;
use wiki_kernel::load_runtime_config;

#[test]
fn parses_provider_config_with_env_style_api_key() {
    let temp = tempdir().expect("tempdir");
    let config_path = temp.path().join("wiki-config.toml");

    // SAFETY: test process controls the temp key for the duration of this test.
    unsafe {
        env::set_var("TEST_EMBEDDING_KEY", "secret-token");
    }

    fs::write(
        &config_path,
        r#"
[retrieval.keyword]
enabled = true
top_k = 12

[retrieval.vector]
enabled = true
base_url = "https://api.example.com/v1"
api_key = "env:TEST_EMBEDDING_KEY"
model = "embedding-small"
timeout_ms = 5000
batch_size = 8
top_k = 10

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 6
"#,
    )
    .expect("write config");

    let config = load_runtime_config(&config_path).expect("load config");

    assert!(config.retrieval.keyword.enabled);
    assert_eq!(config.retrieval.keyword.top_k, 12);
    assert_eq!(config.retrieval.vector.api_key.as_deref(), Some("secret-token"));
    assert_eq!(config.retrieval.vector.model, "embedding-small");
    assert_eq!(config.retrieval.graph.walk_depth, 2);
    assert_eq!(config.retrieval.graph.max_neighbors, 32);
}

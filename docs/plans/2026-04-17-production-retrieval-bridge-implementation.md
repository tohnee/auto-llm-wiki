# Production Retrieval And Bridge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `auto-llm-wiki` 接入生产级的真实检索 provider 和本地图桥，实现 FTS5、embeddings、graph walk、provider health 与索引恢复工作流。

**Architecture:** 在现有 SQLite 状态层上扩展 FTS5、embedding、graph 与 provider 运行记录；`wiki-kernel` 改为基于 provider trait 的三路真实召回；`wiki-cli` 暴露索引同步、重建与健康检查命令。系统坚持“部分失败可降级，状态落地可恢复”的原则。

**Tech Stack:** Rust workspace, `rusqlite` with FTS5, `reqwest` blocking client, `serde`, `serde_json`, `clap`, `sha2`, `uuid`, `chrono`, `tempfile`

---

### Task 1: Provider Config And Core Contracts

**Files:**
- Create: `crates/wiki-core/src/provider.rs`
- Modify: `crates/wiki-core/src/lib.rs`
- Create: `crates/wiki-kernel/src/config.rs`
- Modify: `crates/wiki-kernel/src/lib.rs`
- Test: `crates/wiki-kernel/tests/provider_config.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn parses_provider_config_with_env_style_api_key() {
    // config loader returns expected retrieval settings
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel parses_provider_config_with_env_style_api_key -- --exact`
Expected: FAIL because config and provider contracts do not exist

**Step 3: Write minimal implementation**

实现：
- provider hit / health / status 结构
- retrieval config 结构
- toml 配置加载器
- `env:KEY` 解析辅助

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-kernel provider_config -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-core crates/wiki-kernel
git commit -m "feat: add provider contracts and config loader"
```

### Task 2: FTS5 And Provider State Storage

**Files:**
- Modify: `crates/wiki-storage/src/schema.rs`
- Modify: `crates/wiki-storage/src/sqlite.rs`
- Create: `crates/wiki-storage/src/indexing.rs`
- Test: `crates/wiki-storage/tests/fts_and_provider_state.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn storing_claim_updates_fts_and_marks_embedding_pending() {
    // claim write creates fts row and provider state row
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-storage storing_claim_updates_fts_and_marks_embedding_pending -- --exact`
Expected: FAIL because FTS5 and embedding state are not implemented

**Step 3: Write minimal implementation**

实现：
- `claim_fts`
- `claim_embeddings`
- `graph_nodes`
- `graph_edges`
- `provider_runs`
- claim 写入时 FTS upsert 和 embedding pending 状态

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-storage fts_and_provider_state -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-storage
git commit -m "feat: add fts and provider state storage"
```

### Task 3: Real Keyword Retrieval

**Files:**
- Create: `crates/wiki-kernel/src/providers/keyword.rs`
- Modify: `crates/wiki-kernel/src/retrieval.rs`
- Modify: `crates/wiki-kernel/src/engine.rs`
- Test: `crates/wiki-kernel/tests/keyword_retrieval.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn fts_keyword_retriever_returns_ranked_claims() {
    // query over stored claims returns fts-backed ranked hits
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel fts_keyword_retriever_returns_ranked_claims -- --exact`
Expected: FAIL because keyword provider is not implemented

**Step 3: Write minimal implementation**

实现：
- `KeywordRetriever` trait
- `SqliteFtsRetriever`
- FTS hit 到 rank list 的转换
- 接入 engine query 流程

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-kernel keyword_retrieval -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-kernel
git commit -m "feat: add sqlite fts keyword retrieval"
```

### Task 4: Embedding Provider And Vector Retrieval

**Files:**
- Create: `crates/wiki-kernel/src/providers/embedding.rs`
- Modify: `crates/wiki-kernel/src/engine.rs`
- Modify: `crates/wiki-storage/src/sqlite.rs`
- Modify: `crates/wiki-kernel/Cargo.toml`
- Test: `crates/wiki-kernel/tests/vector_retrieval.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn vector_retrieval_uses_cached_embeddings_and_cosine_similarity() {
    // query embedding ranks nearest claims
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel vector_retrieval_uses_cached_embeddings_and_cosine_similarity -- --exact`
Expected: FAIL because embedding provider and vector retrieval do not exist

**Step 3: Write minimal implementation**

实现：
- OpenAI-compatible embeddings client
- content hash 与 embedding 缓存
- cosine similarity
- pending/ready/failed 状态流转
- query 时 vector 路径

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-kernel vector_retrieval -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-kernel crates/wiki-storage
git commit -m "feat: add embeddings provider and vector retrieval"
```

### Task 5: Mempalace Graph Store And Retrieval

**Files:**
- Replace: `crates/wiki-mempalace-bridge/src/lib.rs`
- Create: `crates/wiki-mempalace-bridge/src/store.rs`
- Create: `crates/wiki-mempalace-bridge/src/retriever.rs`
- Modify: `crates/wiki-kernel/src/engine.rs`
- Test: `crates/wiki-kernel/tests/graph_retrieval.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn graph_retrieval_returns_claims_connected_to_query_concepts() {
    // graph walk reaches related claims through page/source/concept edges
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel graph_retrieval_returns_claims_connected_to_query_concepts -- --exact`
Expected: FAIL because graph bridge is still placeholder

**Step 3: Write minimal implementation**

实现：
- graph node / edge upsert
- claim / page / source graph sync
- graph walk query
- graph retriever trait 实现

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-kernel graph_retrieval -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-mempalace-bridge crates/wiki-kernel
git commit -m "feat: add mempalace graph bridge and retrieval"
```

### Task 6: Sync, Rebuild And Health Commands

**Files:**
- Modify: `crates/wiki-cli/src/args.rs`
- Modify: `crates/wiki-cli/src/main.rs`
- Modify: `crates/wiki-kernel/src/engine.rs`
- Test: `crates/wiki-cli/tests/provider_ops.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn sync_index_and_provider_health_commands_succeed() {
    // commands run and emit machine-readable output
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-cli sync_index_and_provider_health_commands_succeed -- --exact`
Expected: FAIL because commands do not exist

**Step 3: Write minimal implementation**

实现命令：
- `sync-index`
- `rebuild-fts`
- `rebuild-graph`
- `provider-health`

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-cli provider_ops -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-cli crates/wiki-kernel
git commit -m "feat: add provider sync rebuild and health commands"
```

### Task 7: Degradation And Recovery Verification

**Files:**
- Create: `crates/wiki-kernel/tests/degradation.rs`
- Modify: `scripts/e2e.sh`
- Modify: `README.md`

**Step 1: Write the failing test**

```rust
#[test]
fn query_degrades_when_embedding_provider_is_unavailable() {
    // query still succeeds with keyword + graph paths
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel query_degrades_when_embedding_provider_is_unavailable -- --exact`
Expected: FAIL because degradation flow is not verified

**Step 3: Write minimal implementation**

实现：
- degradation metadata
- provider run logging
- e2e 脚本扩展到 sync-index / provider-health
- README 文档更新

**Step 4: Run test to verify it passes**

Run:
- `cargo test`
- `cargo run -p wiki-cli -- --help`
- `bash scripts/e2e.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add .
git commit -m "feat: verify production retrieval degradation and recovery"
```

# Auto LLM Wiki Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 构建一个独立的 Rust workspace，复刻文章中的 `llm-wiki` 核心架构、CLI 工作流、SQLite/outbox/audit、三路检索融合和 Markdown wiki 投影能力。

**Architecture:** `wiki-core` 承载纯领域模型和算法；`wiki-storage` 管理 SQLite 状态、审计和 outbox；`wiki-kernel` 编排 ingest/query/lint/wiki projection；`wiki-mempalace-bridge` 定义外部图谱消费接口；`wiki-cli` 暴露命令。检索依赖通过 trait 注入，默认提供 stub 实现以保证端到端流程可运行。

**Tech Stack:** Rust workspace, `clap`, `serde`, `serde_json`, `rusqlite`, `uuid`, `chrono`, `thiserror`, `tempfile`

---

### Task 1: Workspace Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `crates/wiki-core/Cargo.toml`
- Create: `crates/wiki-kernel/Cargo.toml`
- Create: `crates/wiki-storage/Cargo.toml`
- Create: `crates/wiki-mempalace-bridge/Cargo.toml`
- Create: `crates/wiki-cli/Cargo.toml`
- Create: `crates/*/src/*.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn workspace_smoke() {
    assert_eq!(2 + 2, 4);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test`
Expected: FAIL because workspace and crates do not exist yet

**Step 3: Write minimal implementation**

创建 workspace、crate 清单、基础模块和 smoke test。

**Step 4: Run test to verify it passes**

Run: `cargo test`
Expected: PASS for skeleton tests

**Step 5: Commit**

```bash
git add .
git commit -m "chore: initialize auto llm wiki workspace"
```

### Task 2: Core Domain Model

**Files:**
- Create: `crates/wiki-core/src/lib.rs`
- Create: `crates/wiki-core/src/claim.rs`
- Create: `crates/wiki-core/src/event.rs`
- Create: `crates/wiki-core/src/query.rs`
- Create: `crates/wiki-core/src/lint.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn superseding_claim_marks_old_claim_stale() {
    // old claim becomes stale and new claim points to old id
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-core superseding_claim_marks_old_claim_stale -- --exact`
Expected: FAIL with missing types/functions

**Step 3: Write minimal implementation**

实现：
- `Claim`, `ClaimId`, `MemoryTier`, `SourceRecord`
- `Claim::supersede`
- retention strength 计算
- RRF 融合
- lint issue 模型

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-core`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-core
git commit -m "feat: add core claim and retrieval models"
```

### Task 3: Storage and Audit

**Files:**
- Create: `crates/wiki-storage/src/lib.rs`
- Create: `crates/wiki-storage/src/schema.rs`
- Create: `crates/wiki-storage/src/sqlite.rs`
- Create: `crates/wiki-storage/src/repository.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn storing_claim_emits_outbox_and_audit_records() {
    // persist claim then assert state, outbox, audit
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-storage storing_claim_emits_outbox_and_audit_records -- --exact`
Expected: FAIL because repository does not exist

**Step 3: Write minimal implementation**

实现 SQLite schema 与仓储：
- claims / sources / pages
- wiki_outbox
- outbox_consumers
- audit_records

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-storage`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-storage
git commit -m "feat: add sqlite storage with outbox and audit"
```

### Task 4: Kernel Workflows

**Files:**
- Create: `crates/wiki-kernel/src/lib.rs`
- Create: `crates/wiki-kernel/src/engine.rs`
- Create: `crates/wiki-kernel/src/wiki.rs`
- Create: `crates/wiki-kernel/src/lint.rs`
- Create: `crates/wiki-kernel/src/retrieval.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn query_flow_writes_analysis_page_when_requested() {
    // arrange stub retrievers + repo and assert wiki page created
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-kernel query_flow_writes_analysis_page_when_requested -- --exact`
Expected: FAIL due to missing engine

**Step 3: Write minimal implementation**

实现：
- ingest pipeline
- query application service
- markdown wiki writer
- lint runner
- crystallize / page writing helpers

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-kernel`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-kernel
git commit -m "feat: add kernel workflows and wiki projection"
```

### Task 5: CLI Surface

**Files:**
- Create: `crates/wiki-cli/src/main.rs`
- Create: `crates/wiki-cli/src/args.rs`
- Create: `crates/wiki-cli/tests/cli_e2e.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn ingest_query_lint_flow_succeeds() {
    // invoke app commands against temp dir
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wiki-cli ingest_query_lint_flow_succeeds -- --exact`
Expected: FAIL because CLI commands are missing

**Step 3: Write minimal implementation**

实现命令：
- `ingest`
- `file-claim`
- `supersede`
- `query`
- `lint`
- `outbox export`
- `outbox ack`
- `llm-smoke`

**Step 4: Run test to verify it passes**

Run: `cargo test -p wiki-cli`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/wiki-cli
git commit -m "feat: add wiki cli workflows"
```

### Task 6: Documentation and Final Verification

**Files:**
- Create: `README.md`
- Create: `scripts/e2e.sh`
- Modify: `task_plan.md`
- Modify: `findings.md`
- Modify: `progress.md`

**Step 1: Write the failing test**

```bash
cargo test
```

**Step 2: Run test to verify it fails**

Expected: surface any integration gap

**Step 3: Write minimal implementation**

补齐 README、示例命令、e2e 脚本、默认配置说明。

**Step 4: Run test to verify it passes**

Run:
- `cargo test`
- `cargo run -p wiki-cli -- --help`
- `bash scripts/e2e.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add .
git commit -m "docs: add usage and verification flow"
```

# Production Retrieval And Bridge Design

## Goal

将 `auto-llm-wiki` 从“启发式可运行原型”升级为“可配置、可降级、可恢复、可观测”的生产级知识引擎，重点补齐三类真实能力：

- 基于 SQLite FTS5 的真实关键词召回
- 基于 OpenAI-compatible embeddings API 的真实向量召回
- 基于本地图结构桥接的真实 graph walk 召回与同步

## Scope

本阶段覆盖：

- provider 抽象与配置协议
- SQLite schema 扩展：FTS5、embedding、graph、本地 provider 状态
- `wiki-mempalace-bridge` 的本地图桥实现
- `wiki-kernel` 的真实三路召回编排与降级策略
- `wiki-cli` 的运维命令：`sync-index`、`rebuild-fts`、`rebuild-graph`、`provider-health`
- 端到端测试、降级测试、恢复测试

本阶段不覆盖：

- 外部图数据库
- 外部向量数据库
- 真正的 LLM 自动抽 claim
- 分布式任务调度

## Why This Design

当前系统已经具备 `Claim` 生命周期、SQLite 状态、outbox、审计与 wiki 投影，但检索仍是启发式 stub，`wiki-mempalace-bridge` 仍为空壳。要达到生产级，最重要的不是一次性换成重型基础设施，而是在当前架构上补齐真实 provider、降级策略和恢复链路。

选择 `SQLite FTS5 + OpenAI-compatible embeddings + 本地图桥` 的原因：

- 与当前 `SQLite + local wiki dir` 架构天然兼容
- 本地开发和 CI 成本低
- 不要求引入 Qdrant / Neo4j 等额外运行时
- 可以先把“真实能力链路”跑通，再平滑替换为外部后端

## Target Architecture

```text
Sources / Claims / Pages
          |
          v
   SqliteWikiRepository
          |
          +--> claims / sources / pages
          +--> claim_fts (FTS5)
          +--> claim_embeddings
          +--> graph_nodes / graph_edges
          +--> provider_runs
          +--> wiki_outbox / audit_records
          |
          v
      WikiEngine
          |
          +--> KeywordRetriever (FTS5)
          +--> VectorRetriever (Embeddings API + local similarity)
          +--> GraphRetriever (local mempalace bridge)
          |
          +--> RRF + retention_strength
          |
          +--> Markdown wiki projection
```

## Provider Model

### Retrieval Provider Traits

`wiki-kernel` 中引入三个 trait：

- `KeywordRetriever`
- `VectorRetriever`
- `GraphRetriever`

统一返回结构：

```rust
pub struct ProviderHit {
    pub claim_id: ClaimId,
    pub raw_score: f64,
    pub provider_name: String,
    pub latency_ms: u64,
    pub degraded_reason: Option<String>,
}
```

统一 health 结构：

```rust
pub struct ProviderHealth {
    pub provider_name: String,
    pub status: HealthStatus,
    pub message: String,
    pub checked_at: DateTime<Utc>,
}
```

### Concrete Providers

- `SqliteFtsRetriever`
  - 基于 FTS5 查询 `claim_fts`
  - 使用 `bm25(claim_fts)` 排序
  - 返回 top-k claim ids

- `OpenAiEmbeddingProvider`
  - 调 OpenAI-compatible `/embeddings`
  - 提供 query embedding 与 claim embedding 生成
  - 支持 timeout、batch、重试、错误归档

- `SqliteVectorRetriever`
  - 从 `claim_embeddings` 读取向量
  - 在 Rust 侧计算 cosine similarity
  - 不依赖外部向量数据库

- `MempalaceGraphStore`
  - 使用 `graph_nodes` / `graph_edges`
  - 支持按 `claim/page/source/concept` 建点
  - 支持 `mentions/derived_from/supersedes/linked_to` 建边

- `MempalaceGraphRetriever`
  - 从 query token 映射起始节点
  - 有限深度 walk
  - 收集可回到 claim 的节点并聚合得分

## Storage Design

### Existing Tables

保留并继续使用：

- `claims`
- `sources`
- `pages`
- `wiki_outbox`
- `outbox_consumers`
- `audit_records`

### New Tables

#### `claim_fts`

```sql
CREATE VIRTUAL TABLE claim_fts USING fts5(
  claim_id UNINDEXED,
  text,
  keywords,
  content='',
  tokenize='unicode61'
);
```

用途：

- 提供真实关键词召回
- 存储 claim 文本与可选关键词串

#### `claim_embeddings`

字段：

- `claim_id`
- `model`
- `dim`
- `vector_json`
- `content_hash`
- `status` (`pending|ready|failed`)
- `last_error`
- `embedded_at`
- `updated_at`

用途：

- 缓存 claim embedding
- 支持内容变更探测和重建
- 支持失败恢复

#### `graph_nodes`

字段：

- `node_id`
- `node_type`
- `external_ref`
- `label`
- `payload_json`
- `updated_at`

#### `graph_edges`

字段：

- `edge_id`
- `from_node`
- `to_node`
- `edge_type`
- `weight`
- `payload_json`
- `updated_at`

用途：

- 提供本地 mempalace 图存储
- 支持 graph walk 和全量重建

#### `provider_runs`

字段：

- `run_id`
- `provider_name`
- `operation`
- `target_ref`
- `status`
- `latency_ms`
- `error_message`
- `payload_json`
- `created_at`

用途：

- 记录 embedding、graph sync、query provider 调用
- 作为故障追踪与降级审计来源

## Write Path

### Ingest / File Claim

1. source / claim 先写权威状态表
2. claim 写入 `claims`
3. 同步更新 FTS5
4. embedding 状态标记为 `pending`
5. graph 状态标记为 `dirty`
6. 写 outbox 和 audit

### Supersede

1. 原 claim 标记 stale
2. 新 claim 写入
3. FTS5 更新旧记录与新记录
4. embedding 仅对新 claim 标记 `pending`
5. graph 增加 `supersedes` 边
6. 写 outbox 和 audit

### Page Write

1. page 内容落盘
2. `pages` 表 upsert
3. 图中更新 page node 与 `linked_to` 边
4. 写 outbox 和 audit

## Query Path

1. `WikiEngine` 收到 query
2. 并行执行：
   - FTS5 keyword retrieval
   - vector retrieval
   - graph walk retrieval
3. 每一路产出 `ProviderHit`
4. 转成 rank list 后做 RRF
5. 乘以 `retention_strength`
6. 输出 top-k claims
7. 若启用 `--write-page`，投影 analysis page
8. query provider 结果与降级信息写入 `provider_runs` 和 audit

## Degradation Strategy

### Principle

部分失败不拖垮整体 query。

### Rules

- FTS5 失败：
  - 继续 vector + graph
  - 记录 degraded reason

- embedding provider 失败：
  - vector 路径跳过
  - 若已有旧 embedding，可选择继续使用缓存

- graph walk 失败：
  - 继续 FTS5 + vector

- 三路均失败：
  - query 返回错误
  - 写 audit 与 provider_runs

## Recovery Strategy

### Embedding Recovery

- `claim_embeddings.status` 记录 `pending|ready|failed`
- `sync-index` 批量扫描 `pending|failed`
- 成功后更新向量与状态
- 失败后记录 `last_error`

### Graph Recovery

- graph 同步失败不影响 claim 落库
- `rebuild-graph` 从 `claims/pages/sources` 全量重建图
- `sync-index` 也可按 dirty 状态增量补偿

### FTS Recovery

- `rebuild-fts` 全量重建 `claim_fts`
- 适用于 schema 迁移、索引损坏或逻辑修复

## Configuration

统一配置文件：`wiki-config.toml`

```toml
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "https://api.deepseek.com/v1"
api_key = "env:DEEPSEEK_API_KEY"
model = "text-embedding-3-small"
timeout_ms = 30000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
```

规则：

- `api_key` 支持 `env:KEY_NAME`
- 缺配置时使用保守默认值
- provider health 命令可以验证配置有效性

## CLI Extensions

新增命令：

- `sync-index`
  - 补 embedding 和 graph 增量同步

- `rebuild-fts`
  - 全量重建 FTS5

- `rebuild-graph`
  - 全量重建 graph tables

- `provider-health`
  - 检查 FTS5、embedding API、graph store 状态

## Observability

### Audit

继续写：

- ingest
- claim upsert
- supersede
- query
- lint
- page write

额外补充：

- provider degrade
- sync-index runs
- rebuild runs
- provider health checks

### Outbox

保留现有 outbox，但 provider 内部失败不直接写业务 outbox；统一写 `provider_runs` 与 audit，避免污染业务事件流。

## Testing Strategy

### Unit Tests

- FTS result rank mapping
- cosine similarity
- graph walk scoring
- query degradation decision
- config parsing

### Storage Tests

- claim 写入同步 FTS
- embedding status 流转
- graph node/edge 重建
- provider_runs 记录完整

### Integration Tests

- `ingest -> sync-index -> query`
- 关闭 embedding provider 后 query 降级仍成功
- `rebuild-fts` 后结果一致
- `rebuild-graph` 后 graph retrieval 恢复

### E2E

- `scripts/e2e.sh` 扩展为：
  - ingest
  - sync-index
  - query
  - lint
  - outbox export / ack
  - provider-health

## Rollout Plan

1. 先引入 provider trait 和 config，不改变现有 CLI 行为
2. 再扩 schema 与 repository
3. 接入 FTS5 keyword retrieval
4. 接入 embeddings 与本地 vector retrieval
5. 实现 mempalace graph store / retriever
6. 增加 sync / rebuild / health 命令
7. 扩展测试与 e2e

## Open Risks

- OpenAI-compatible embeddings provider 的模型维度需配置化，不能写死
- SQLite 存向量在数据量大时性能有限，但本阶段可接受
- graph token 到 node 的映射规则需要避免过度召回
- FTS5 在不同 SQLite 编译特性下需要验证可用性

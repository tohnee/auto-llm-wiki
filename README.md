# Auto LLM Wiki

`auto-llm-wiki` 是一个基于 Rust 的 LLM Wiki / 持续知识编译系统。

它不是传统“把文档丢给 RAG，然后每次 query 临时检索”的做法，而是把知识拆成可持续演化的 `Claim`，落到 SQLite 状态、Markdown Wiki、全文索引、向量索引、图结构和审计事件中，让系统具备以下能力：

- 原始资料只 ingest 一次
- 知识断言可持续维护、可 supersede、可追溯
- 检索走 `Keyword + Vector + Graph` 三路召回
- 三路结果通过 `RRF * retention_strength` 融合
- 所有关键写操作都留下 outbox 和 audit 轨迹
- Wiki 页面可直接投影到本地 `wiki/` 目录

## 系统目标

这个工程要解决的核心问题不是“检索到文档”，而是“让知识可积累、可演进、可审计、可恢复”。

系统关注四类长期能力：

- **持续记忆**：知识不是一次性回答，而是状态化、结构化、可更新的断言集合
- **多路检索**：关键词、向量、图结构各自召回，再统一融合
- **工程可观测性**：provider 调用、降级、索引构建、查询结果都可以追踪
- **可恢复性**：FTS、embedding、graph 都支持重建与重新同步

## 核心特性

### 1. Claim 生命周期

每条知识断言具备结构化生命周期字段：

- `tier`: `Working | Episodic | Semantic | Procedural`
- `confidence`
- `quality_score`
- `supersedes`
- `stale`
- `access_count`
- `created_at / updated_at`

这意味着旧结论不会被简单覆盖，而是会留下被 supersede 的演化关系。

### 2. 三路检索

当前系统的生产级检索链路包括：

- **SQLite FTS5**：真实关键词召回
- **OpenAI-compatible embeddings**：真实向量生成
- **Local cosine similarity**：本地向量相似度排序
- **Mempalace local graph bridge**：基于本地图结构的 graph walk 召回

最终结果会进入统一融合层：

- `KeywordRetriever`
- `VectorRetriever`
- `GraphRetriever`
- `RRF`
- `retention_strength`

### 3. Wiki 投影

系统会把状态投影到本地 Wiki 目录：

- `wiki/index.md`
- `wiki/log.md`
- `wiki/pages/`
- `wiki/concepts/`
- `wiki/reports/`

因此可以直接用 Obsidian、普通 Markdown 阅读器或静态文件方式浏览。

### 4. 审计与事件

系统在多个层面保留可追踪记录：

- `audit_records`
- `wiki_outbox`
- `outbox_consumers`
- `provider_runs`

其中 `provider_runs` 用于记录：

- provider 名称
- 操作类型
- 目标对象
- 执行状态
- 错误信息
- 降级路径

### 5. 索引恢复与运维命令

系统已支持以下生产级维护动作：

- `sync-index`
- `rebuild-fts`
- `rebuild-graph`
- `provider-health`

## Workspace 结构

```text
auto-llm-wiki/
├── crates/
│   ├── wiki-core
│   ├── wiki-storage
│   ├── wiki-kernel
│   ├── wiki-mempalace-bridge
│   └── wiki-cli
├── docs/plans/
├── scripts/
└── README.md
```

各 crate 职责如下：

- `wiki-core`
  - 领域模型
  - `Claim` 生命周期
  - RRF / retention 逻辑
  - lint issue / event / provider 基础类型

- `wiki-storage`
  - SQLite 持久化
  - FTS5
  - embedding 缓存
  - graph nodes / edges
  - outbox / audit / provider_runs

- `wiki-kernel`
  - ingest / query / lint / wiki projection
  - keyword / vector / graph provider 编排
  - sync-index / rebuild / provider-health 内核逻辑

- `wiki-mempalace-bridge`
  - 本地图桥
  - claim / source / page -> graph node/edge 同步
  - graph rebuild

- `wiki-cli`
  - 统一命令行入口
  - config 加载
  - 运行时 provider 装配

## 架构总览

```text
Raw Sources / Claims / Pages
          |
          v
   SqliteWikiRepository
          |
          +--> claims
          +--> sources
          +--> pages
          +--> claim_fts
          +--> claim_embeddings
          +--> graph_nodes / graph_edges
          +--> provider_runs
          +--> wiki_outbox / audit_records
          |
          v
       WikiEngine
          |
          +--> SqliteFtsRetriever
          +--> OpenAiCompatibleEmbeddingClient
          +--> CosineVectorRetriever
          +--> MempalaceGraphRetriever
          |
          +--> RRF + retention_strength
          |
          +--> Markdown wiki projection
```

## 主要数据表

系统当前主要依赖以下状态表和索引表：

- `claims`
- `sources`
- `pages`
- `claim_fts`
- `claim_embeddings`
- `graph_nodes`
- `graph_edges`
- `provider_runs`
- `wiki_outbox`
- `outbox_consumers`
- `audit_records`

### `claim_fts`

用途：

- 支持关键词召回
- 为 query 提供真实 FTS5 rank list

### `claim_embeddings`

用途：

- 存储 embedding 缓存
- 跟踪 `pending / ready / failed`
- 为 `sync-index` 和 vector retrieval 提供状态支持

### `graph_nodes / graph_edges`

用途：

- 表达 `claim / source / page / concept`
- 支持本地 graph walk 检索
- 支持 `rebuild-graph`

### `provider_runs`

用途：

- 记录 query / sync-index 中 provider 的执行情况
- 记录降级与失败
- 为排障和运维提供证据

## 检索流程

一次 query 的核心流程如下：

1. 加载 `RuntimeConfig`
2. 走 `SqliteFtsRetriever`
3. 若 vector 开启，则自动构造 `OpenAiCompatibleEmbeddingClient`
4. 若能拿到 query embedding，则走 `CosineVectorRetriever`
5. 若 graph 开启，则走 `MempalaceGraphRetriever`
6. 三路结果融合
7. 乘以 `retention_strength`
8. 产出 top-k claims
9. 如果指定 `--write-page`，写出 Markdown 页面
10. 记录 `provider_runs`、audit、outbox

### 自动 vector provider

`query()` 当前已经支持自动 real vector provider：

- 如果 `wiki-config.toml` 中 `retrieval.vector.enabled = true`
- 且 `base_url` 非空
- 则 `WikiEngine::query()` 会自动创建真实 embeddings client

也就是说，vector 检索不再只是 CLI 手动组装测试入口，而是正式进入系统默认 query 路径。

### 降级策略

如果 vector provider 不可用：

- query 不会直接失败
- 会降级为 `keyword + graph`
- 会写入 `provider_runs`
- 会追加 audit 降级摘要

这保证了系统具备生产级的“部分失败不整体失败”能力。

## 配置文件

统一配置文件为 `wiki-config.toml`。

示例：

```toml
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "https://api.example.com/v1"
api_key = "env:EMBEDDING_API_KEY"
model = "embedding-small"
timeout_ms = 30000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
```

说明：

- `api_key` 支持 `env:VAR_NAME`
- 未传 `--config` 时会使用保守默认值
- `provider-health` 会基于当前配置与状态表给出健康信息

## 快速开始

### 1. 运行测试

```bash
cargo test
```

### 2. 准备配置

创建 `wiki-config.toml`：

```toml
[retrieval.keyword]
enabled = true
top_k = 20

[retrieval.vector]
enabled = true
base_url = "https://api.example.com/v1"
api_key = "env:EMBEDDING_API_KEY"
model = "embedding-small"
timeout_ms = 30000
batch_size = 16
top_k = 20

[retrieval.graph]
enabled = true
walk_depth = 2
max_neighbors = 32
top_k = 20
```

### 3. Ingest 原始资料

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  ingest "file:///notes/redis.md" "Redis default TTL is 3600 seconds" \
  --scope private:me
```

### 4. 写入额外 claim

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  file-claim "Redis is used as a cache" \
  --tier semantic
```

### 5. 构建向量和图索引

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  sync-index
```

### 6. 查询并写页面

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  query "Redis TTL" \
  --write-page \
  --page-title analysis-redis
```

### 7. 跑 lint

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  lint
```

### 8. 查看 provider 状态

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --config wiki-config.toml \
  provider-health
```

## CLI 命令总览

### 数据写入

- `ingest <source> <content> [--scope]`
- `file-claim <text> [--tier]`
- `supersede <claim_id> <text> [--confidence] [--quality-score]`

### 查询与分析

- `query <query> [--write-page] [--page-title]`
- `lint`

### 索引与运维

- `sync-index`
- `rebuild-fts`
- `rebuild-graph`
- `provider-health`

### 事件与调试

- `outbox export [--consumer]`
- `outbox ack <event_id> [--consumer]`
- `llm-smoke [--config] [--prompt]`

## Wiki 输出目录

生成的 `wiki/` 目录通常包含：

```text
wiki/
├── index.md
├── log.md
├── pages/
├── concepts/
└── reports/
```

其中：

- `index.md`：页面入口索引
- `log.md`：投影和报告写入日志
- `pages/`：分析页、主题页
- `concepts/`：概念型页面保留目录
- `reports/`：lint 等结构化报告

## 审计、事件与可观测性

### Audit

审计记录覆盖：

- ingest
- claim upsert
- supersede
- query
- lint
- page write
- sync-index
- provider degradation

### Outbox

系统写操作会生成 outbox 事件，支持：

- 导出未消费事件
- 按 consumer ack
- 增量消费

### Provider Runs

`provider_runs` 当前记录：

- `provider_name`
- `operation`
- `target_ref`
- `status`
- `error_message`

已覆盖的操作包括：

- `query`
- `sync-index`

## 验证方式

### 单元 / 集成测试

```bash
cargo test
```

### 端到端验证

```bash
bash scripts/e2e.sh
```

当前 `scripts/e2e.sh` 已覆盖：

- ingest
- file-claim
- sync-index
- query
- lint
- provider-health
- rebuild-fts
- rebuild-graph
- outbox export / ack
- llm-smoke

并且脚本会启动本地 mock embeddings HTTP 服务，真实覆盖自动 vector query 链路。

## 当前已实现的生产级能力

当前系统已经具备以下“不是占位，而是已进入代码路径和验证路径”的能力：

- SQLite FTS5 关键词召回
- OpenAI-compatible embeddings provider
- 本地 cosine similarity vector retrieval
- 本地图桥 graph sync 与 graph retrieval
- query 自动 real vector provider 装配
- vector provider 失败降级
- provider_runs 持久化
- sync-index / provider-health / rebuild-fts / rebuild-graph
- Markdown wiki projection
- outbox export / ack
- lint 与 lint report

## 当前限制

虽然系统已经具备生产级骨架，但仍有一些明确边界：

- 还没有接外部图数据库
- 还没有接外部向量数据库
- embedding 检索当前仍以本地 SQLite 缓存 + Rust cosine similarity 为主
- `provider_runs` 已记录核心状态，但 `latency_ms` 与更细 payload 仍可进一步丰富
- query 返回结果里还没有完整暴露每个 provider 的参与细节
- 自动 claim 提取和 LLM 主动知识整理尚未实现

## 后续可继续强化的方向

- 给 `provider-health` 增加主动 HTTP 探活
- 给 `provider_runs` 增加更细的延迟与 payload
- 在 query 输出中直接暴露 provider 命中与降级详情
- 引入外部向量库作为可替换后端
- 引入外部图数据库或远程 mempalace 服务
- 加入自动 claim 抽取与 session crystallization

## 推荐阅读顺序

如果你要快速理解代码，建议按这个顺序读：

1. `README.md`
2. `crates/wiki-core`
3. `crates/wiki-storage`
4. `crates/wiki-kernel`
5. `crates/wiki-mempalace-bridge`
6. `crates/wiki-cli`
7. `docs/plans/2026-04-17-production-retrieval-bridge-design.md`

## 许可与备注

- 当前仓库以工程复刻与能力验证为目标
- 代码结构已面向继续扩展真实 provider 和外部 bridge
- 适合作为 LLM Wiki、长期记忆系统、可审计知识引擎的基础骨架

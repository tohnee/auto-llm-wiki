# Findings

## Article Capability Map

- 基础形态不是普通 RAG，而是 `Raw Sources -> WikiKernelEngine -> SQLite State -> Markdown Wiki`。
- workspace 目标分五层：
  - `wiki-core`: 纯领域模型和算法，零 IO
  - `wiki-kernel`: 编排引擎、投影、应用服务
  - `wiki-storage`: SQLite、outbox、审计
  - `wiki-mempalace-bridge`: 外部知识图谱桥接
  - `wiki-cli`: 终端入口
- `Claim` 至少包含：`tier`、`confidence`、`quality_score`、`supersedes`、`stale`、`access_count`。
- 检索是三路并行召回：BM25、Vector、Graph；融合规则是 `RRF * retention_strength`。
- 每次写操作都要产生 outbox 事件，可按 offset 拉取并 ack。
- 系统内置 lint，至少检查：
  - `page.broken_wikilink`
  - `page.orphan`
  - `claim.stale`
  - `xref.missing`
- 需要完整审计轨迹：ingest、claim upsert、supersede、query、lint、crystallize。

## Implementation Strategy

- 核心领域逻辑先做纯函数，避免一开始把数据库和 CLI 耦合进去。
- 检索器先用 trait 抽象，默认实现为 stub/in-memory，以满足“能力对齐 + 可运行”。
- SQLite 层同时承担：
  - 当前状态表
  - 事件 outbox
  - 消费者 ack/offets
  - 审计记录
- Markdown 投影作为 kernel 的副作用输出，不放进 core。
- CLI 负责把多个应用服务串起来，并提供 `--sync-wiki` 行为。

## Implemented So Far

- workspace 已建立并能执行 `cargo test`。
- `wiki-core` 已实现：
  - `ClaimId`
  - `MemoryTier`
  - `Claim` / `ClaimReplacement`
  - `retention_strength`
  - `fuse_ranked_results`
  - 基础事件与 lint 数据模型
- `wiki-storage` 已实现：
  - SQLite schema 初始化
  - `SqliteWikiRepository::open_in_memory`
  - `store_claim`
  - `get_claim`
  - `list_outbox`
  - `list_audit_records`
- `wiki-kernel` 已实现：
  - `WikiEngine::ingest`
  - `WikiEngine::file_claim`
  - `WikiEngine::supersede`
  - `WikiEngine::query`
  - `WikiEngine::run_lint`
  - `WikiEngine::export_outbox`
  - `WikiEngine::ack_outbox`
  - wiki layout / page write / report write
  - 启发式三路检索融合
- `wiki-cli` 已实现完整命令面：
  - `ingest`
  - `file-claim`
  - `supersede`
  - `query`
  - `lint`
  - `outbox export`
  - `outbox ack`
  - `llm-smoke`
- 根目录已补齐 `README.md` 和 `scripts/e2e.sh`。

## Risks

- 文章没有完整公开 schema 和表结构，需要根据能力描述自行补齐。
- 真正的 BM25/vector/graph 在线实现超出当前范围，因此必须通过接口层保持可替换性。
- 若 crate 边界设计不稳，后续会在 kernel/storage 之间出现重复模型。

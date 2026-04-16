# Auto LLM Wiki Task Plan

## Goal

构建一个独立的 Rust workspace，复刻文章中的 `llm-wiki` 核心能力：
- 五个 crate 分层：`wiki-core`、`wiki-kernel`、`wiki-storage`、`wiki-mempalace-bridge`、`wiki-cli`
- `Claim` 生命周期建模：tier、confidence、quality_score、supersedes、stale、access_count
- SQLite 持久化、outbox 事件、审计轨迹
- 三路召回接口：BM25、Vector、Graph，使用 RRF + retention strength 融合
- Markdown wiki 投影：`index.md`、`log.md`、`pages/`、`concepts/`、`reports/`
- CLI 命令：`ingest`、`file-claim`、`supersede`、`query`、`lint`、`outbox export/ack`、`llm-smoke`

## Constraints

- 目标是“能力全量对齐”，不是仅做概念验证。
- 外部图谱、真实向量库、真实 LLM provider 做成可插拔接口，并提供可运行 stub。
- 默认使用 ASCII。
- 每个阶段必须有可独立验证的测试或命令。

## Phases

| Phase | Status | Description | Verification |
|---|---|---|---|
| 1 | complete | 建立规划文件与实现计划 | 规划文件存在且内容完整 |
| 2 | complete | 初始化 workspace 与 crate 骨架 | `cargo test` 编译通过 |
| 3 | complete | 实现核心领域模型与纯函数逻辑 | `cargo test -p wiki-core` 通过 |
| 4 | complete | 实现 SQLite、outbox、audit 存储 | `cargo test -p wiki-storage` 通过 |
| 5 | pending | 实现 kernel、wiki 投影、CLI 工作流 | 端到端测试通过 |
| 6 | pending | 运行完整验证并整理交付说明 | 测试与诊断 clean |

## Decisions

- 仓库名：`auto-llm-wiki`
- 语言与架构：纯 Rust workspace
- 数据库：SQLite，优先 `rusqlite` + bundled SQLite 以降低环境依赖
- CLI：`clap`
- 时间与 ID：`chrono`、`uuid`
- 序列化：`serde`、`serde_json`
- 测试策略：先核心单元测试，再存储测试，再 CLI/e2e 测试

## Open Questions

- 无。当前范围已足够开始实现。

## Errors Encountered

| Error | Attempt | Resolution |
|---|---|---|
| 批量 Python 重写 `Cargo.toml` 失败，出现三引号字符串未闭合 | 1 | 放弃同方法，改为逐文件 patch，已解决 |

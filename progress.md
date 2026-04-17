# Progress

## 2026-04-17

- 已阅读并整理文章中的核心架构与实现点。
- 已确认实现落点为新仓库 `auto-llm-wiki`。
- 已确认范围为“能力全量对齐”，外部系统采用可插拔接口 + 可运行 stub。
- 已初始化 git 仓库与 `docs/plans/` 目录。
- 已建立 `.worktrees/implementation` 隔离工作区并切到 `feat/implementation`。
- 已完成 workspace 骨架、`wiki-core` 第一轮领域模型和 `wiki-storage` SQLite/outbox/audit 最小实现。
- 已完成 `wiki-kernel` 的 query/wiki projection/lint 工作流实现。
- 已完成 `wiki-cli` 的命令面实现与集成测试。
- 已补齐 `README.md` 与 `scripts/e2e.sh`。
- 已确认生产级方案：`SQLite FTS5 + OpenAI-compatible embeddings + 本地图桥`。
- 已新增生产级设计文档与 implementation plan。
- 已完成 provider contracts 与运行时配置加载。
- 已完成 FTS5 表、embedding 状态表，以及 claim 写入后的索引/状态同步最小实现。
- 已完成 `SqliteFtsRetriever`，并把 `WikiEngine.query()` 的关键词路径切到真实 FTS5 检索。
- 已完成 OpenAI-compatible embeddings client、本地 cosine similarity retrieval，以及基于 retriever 的 query 入口。
- 已完成 mempalace graph bridge、本地 graph retrieval、`sync-index`、`provider-health`、`rebuild-fts`、`rebuild-graph`。
- 当前阶段：生产级 retrieval/bridge 这一轮实现与验证完成。

## Next

- 对接真实 BM25 / vector / graph provider。
- 扩展 `wiki-mempalace-bridge` 为真实外部知识图谱集成。
- 增加更细的 audit/outbox 消费回归测试。

---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Progress

- `merman-lsp` 已创建，并接入 `tower-lsp` / `lsp-types`。
- 诊断投递已走 `merman-analysis`，Markdown fence 的 host 文档重映射已完成。
- completion 已有最小可用的本地 node id 建议。
- 额外补了 plain Mermaid 文档的 snapshot fence，避免非 Markdown 文档没有 completion 上下文。
- 新增的 crate tests 已通过，当前下一步是补一个真正驱动 LSP 生命周期的 smoke 覆盖，以及 `docs/lsp/` 对外说明。

# Citations

- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [merman-lsp crate](../../../../crates/merman-lsp/src/server.rs)
- [document store](../../../../crates/merman-lsp/src/document_store.rs)

---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

`merman-lsp` 已经从纯骨架推进到可用的 diagnostics + completion + navigation 基线：诊断投递走 `merman-analysis`，Markdown fence 重映射统一，completion 现在覆盖 diagram header、direction、operator、directive、shape 和本地 node id，并且开始使用 snapshot 驱动的替换范围而不是裸插入。`DocumentStore` 与 `CompletionContext` 已经形成可继续深挖的 snapshot seam，当前快照还携带 diagram type 和常见 directive prefix 事实，`server_smoke` 也验证了 initialize/open/change/save 的当前版本诊断发布。`merman-analysis::document::analyze_document` 现在把 CLI lint 和 LSP 收进同一条文档分析 seam，避免在各自适配层重复决定 markdown/plain 分支。

# Details

- `merman-analysis` 统一提供 LSP 诊断位置转换与 Markdown URI 判断。
- `merman-analysis` 新增了 `diagram_type_for_text` 共享 helper，供 LSP snapshot 层复用。
- `merman-analysis` 新增了 `document::analyze_document` 共享入口，供 CLI lint 和 LSP 复用同一条 plain/markdown 文档分析路径。
- `merman-lsp` 的 completion 逻辑已经从 ad hoc 字符串判断收进 snapshot/context，并开始用上下文计算 text edit range。
- `snapshot` 现在存储 fence-level 的 `diagram_type` 与 directive prefix 索引，避免 downstream 再次扫原文。
- `server` 端不再自己猜 Markdown 扩展名，而是复用共享判断。
- `completion`、`document_store`、`diagnostics` 和 `server_smoke` 测试现在覆盖 plain `.mmd` 与 Markdown fence 两条路径，completion 还覆盖了替换范围，document_store 也验证了快照结构事实。
- 共享结构层已经从 hover/documentSymbol 扩到 definition/references/prepareRename/rename，并且有协议级 smoke test 证明这条导航面可用。

# Next Action

决定下一步是继续沿着同一条 structure/navigation seam 做 code actions、linked editing、workspace symbol，还是拆出新的 lint/LSP 产品化计划 slice。

# Citations

- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [merman-lsp crate](../../../../crates/merman-lsp/src/server.rs)
- [document store](../../../../crates/merman-lsp/src/document_store.rs)
- [analysis LSP helpers](../../../../crates/merman-analysis/src/lsp.rs)

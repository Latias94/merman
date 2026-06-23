---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

`merman-lsp` 已经从纯骨架推进到可用的 diagnostics + completion 基线：诊断投递走 `merman-analysis`，Markdown fence 重映射统一，completion 现在覆盖 diagram header、direction、operator、directive、shape 和本地 node id。`DocumentStore` 与 `CompletionContext` 已经形成可继续深挖的 snapshot seam，`server_smoke` 也验证了 initialize/open/change/save 的当前版本诊断发布。

# Details

- `merman-analysis` 统一提供 LSP 诊断位置转换与 Markdown URI 判断。
- `merman-lsp` 的 completion 逻辑已经从 ad hoc 字符串判断收进 snapshot/context。
- `server` 端不再自己猜 Markdown 扩展名，而是复用共享判断。
- `completion`、`document_store`、`diagnostics` 和 `server_smoke` 测试现在覆盖 plain `.mmd` 与 Markdown fence 两条路径。

# Next Action

决定下一步是做 lint 入口、补 completion metadata，还是继续把 LSP snapshot seam 往 hover/symbol 基建方向推进。

# Citations

- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [merman-lsp crate](../../../../crates/merman-lsp/src/server.rs)
- [document store](../../../../crates/merman-lsp/src/document_store.rs)
- [analysis LSP helpers](../../../../crates/merman-analysis/src/lsp.rs)

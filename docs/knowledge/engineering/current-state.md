---
type: Current State
status: active
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: U4 LSP boundary slice passed `cargo test -p merman-lsp --test server_smoke`, `cargo test -p merman-lsp --lib`, `cargo test -p merman-lsp --test diagnostics`, `cargo test -p merman-lsp --test capabilities`, `cargo fmt --all --check`, and `git diff --check`.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, U3 parser spans/fallback ledger, and U4 LSP document-pull/push/workspace/code-action boundary cleanup.
- In progress: U5 VS Code preview controls and interaction polish.
- Blocked: none.
- Next action: inspect the preview HTML/CSS/JS/source-action tests, then make common SVG copy/export actions and viewport behavior match the U5 contract.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)

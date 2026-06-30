---
type: Current State
status: active
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: U6 cleanup passed `cargo check -p merman-analysis --tests`, `cargo test -p merman-analysis --lib fallback_recovery_merge_uses_structured_location_metadata -- --nocapture`, `cargo test -p merman-analysis --lib parse_failure_deduplicates -- --nocapture`, `cargo test -p merman-editor-core --test diagnostics`, `cargo check -p merman-lsp --tests`, `npm run check`, `npm test -- --test-reporter=spec`, `cargo fmt --all --check`, and `git diff --check`.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, U3 parser spans/fallback ledger, U4 LSP document-pull/push/workspace/code-action boundary cleanup, U5 preview/source-action UX polish, and U6 redundant compatibility path cleanup.
- In progress: U7 diagnostics documentation and validation matrix.
- Blocked: none.
- Next action: update LSP capability docs, diagnostics protocol docs, ADR 0070, crate READMEs, and VS Code README so they describe the final ownership model, parser-family span matrix, no numeric Problems codes, no preview quick-fix panel, no pin controls, and no real workspace diagnostics.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)

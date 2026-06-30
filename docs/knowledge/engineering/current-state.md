---
type: Current State
status: complete
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: final gates passed `cargo fmt --all --check`, `git diff --check`, `python3 ~/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`, `cargo check -p merman-analysis -p merman-editor-core -p merman-lsp --tests`, `cargo test -p merman-analysis --lib parse_failure_deduplicates -- --nocapture`, `cargo test -p merman-analysis --lib fallback_recovery_merge_uses_structured_location_metadata -- --nocapture`, `cargo test -p merman-editor-core --test diagnostics`, `cargo test -p merman-lsp --test diagnostics`, `npm run check`, and `npm test -- --test-reporter=spec`.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, U3 parser spans/fallback ledger, U4 LSP document-pull/push/workspace/code-action boundary cleanup, U5 preview/source-action UX polish, U6 redundant compatibility path cleanup, and U7 diagnostics documentation/validation matrix.
- In progress: none.
- Blocked: none.
- Next action: no required follow-up for this plan; optional next work is parser span coverage for Architecture/Kanban render validators or unopened-file workspace diagnostics as a separate plan.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)

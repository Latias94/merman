---
type: Work Progress
status: active
---

# U8 Capability Matrix And Multi-Fence Gates

- Date: 2026-06-27
- Branch: `feat/diagnostics-analysis-contract`
- Goal: keep pushing `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`
  toward mature Mermaid LSP coverage with fewer transport-local heuristics.

## What Changed

- `crates/merman-lsp/tests/capabilities.rs` now checks the checked-in capability matrix document
  and fails if any first-class family row slips away from the mature `Yes` state.
- `crates/merman-lsp/tests/document_store.rs` now verifies a Markdown document with multiple
  Mermaid fences materializes multiple parser-backed fence snapshots instead of collapsing to one.

## Why It Matters

- The public maturity matrix is now guarded by code, not only prose.
- Multi-fence Markdown documents remain a first-class LSP input shape.
- U8 now has two concrete gates in place before the remaining fixture/performance/docs work.

## Verification

- `cargo test -p merman-lsp --test capabilities capability_matrix_document_marks_first_class_families_mature -- --nocapture`
- `cargo test -p merman-lsp --test document_store markdown_documents_create_multiple_mermaid_fences -- --nocapture`
- `cargo test -p merman-analysis text_scan_mindmap_keeps_labels_out_of_node_ids -- --nocapture`
- `cargo fmt --all`
- `cargo fmt --all --check`
- `git diff --check`

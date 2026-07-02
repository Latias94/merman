---
type: Work Progress
status: active
---

# U8 Multi-Fence Completion Gate

- Date: 2026-06-27
- Branch: `feat/diagnostics-analysis-contract`
- Goal: keep pushing `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`
  toward mature Mermaid LSP coverage with fewer transport-local heuristics.

## What Changed

- `crates/merman-lsp/tests/completion.rs` now proves a single Markdown document with multiple
  Mermaid fences keeps completion local to the fence under the cursor.
- The first fence still exposes local flowchart node ids, while the later sequence fence stays in
  its own payload context and does not leak flowchart ids across fence boundaries.

## Why It Matters

- Multi-fence Markdown is now validated at the protocol surface, not only in snapshot extraction.
- Completion no longer relies on a single-fence mental model for Markdown documents.
- U8 now has a more realistic LSP-facing gate for large documents with multiple diagrams.

## Verification

- `cargo test -p merman-lsp --test completion completion_stays_fence_local_across_multiple_markdown_mermaid_blocks -- --nocapture`
- `cargo test -p merman-lsp --test document_store markdown_documents_create_multiple_mermaid_fences -- --nocapture`
- `cargo fmt --all`
- `cargo fmt --all --check`
- `git diff --check`

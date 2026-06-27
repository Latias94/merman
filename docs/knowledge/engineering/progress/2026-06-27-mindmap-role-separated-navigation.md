---
type: Work Progress
status: active
---

# Mindmap Role-Separated Navigation

- Date: 2026-06-27
- Branch: `feat/diagnostics-analysis-contract`
- Goal: keep pushing `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`
  toward mature Mermaid LSP coverage with fewer transport-local heuristics.

## What Changed

- `crates/merman-core/src/diagrams/mindmap/parse.rs` now emits explicit payload symbols for node
  labels in addition to node-id entity symbols, so label text no longer shares navigation state
  with the node id.
- `crates/merman-analysis/src/editor.rs` narrows mindmap text-scan fallback node-id collection to
  structural tokens, keeping label text out of completion ids even when the parser is not
  available.
- `crates/merman-lsp/src/structure.rs` now proves mindmap node ids remain renameable while label
  payload spans are not navigation targets.

## Why It Matters

- Mindmap labels now behave like payloads instead of accidental node ids.
- Rename, references, and hover stay anchored to structural ids rather than visible label text.
- The fallback scan is less heuristic and closer to the parser-backed contract.

## Verification

- `cargo test -p merman-core mindmap_editor_facts_preserve_parser_node_spans -- --nocapture`
- `cargo test -p merman-analysis text_scan_mindmap_keeps_labels_out_of_node_ids -- --nocapture`
- `cargo test -p merman-lsp structure::tests::mindmap_node_ids_are_renameable_and_payloads_are_not_navigation_targets -- --nocapture`
- `cargo test -p merman-lsp --test capabilities product_families_are_parser_backed_and_role_aware -- --nocapture`
- `cargo fmt --all`
- `cargo fmt --all --check`
- `git diff --check`

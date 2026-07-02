---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Class Directive Outline And Fallback

- Date: 2026-06-27
- Goal: tighten class-diagram directive semantics so parser-backed facts and text-scan fallback
  agree on what should participate in completion/navigation.

## What Changed

- `merman-core` now records class-diagram `class` statements as directive prefixes, which lets
  downstream completion contexts treat them like other directive-bearing lines.
- `classDef` symbols in class diagrams were demoted from completion-visible entities to outline
  symbols, so class style definitions no longer leak into node-id completion.
- `merman-analysis` now recognizes `cssClass`, `link`, and `callback` as class-diagram directive
  prefixes in both text-scan fallback and directive completion gating.
- `merman-lsp` now treats the same directive families as directive helpers when it renders the
  generic directive completion item.

## Why It Matters

- Parser-backed semantic facts now line up with the intended editor surface instead of exposing
  class style names as graph nodes.
- The text-scan fallback is narrower and less noisy, which matters when a family drops into the
  recovered / unsupported path.
- This keeps the long-term direction pointed at semantic facts and parser-backed completion rather
  than growing a larger LSP-local heuristic layer.

## Verification

- `cargo test -p merman-core parse_class_editor_facts_preserve_parser_symbol_spans -- --nocapture`
- `cargo test -p merman-analysis text_scan_skips_class_directive_payload_prefixes -- --nocapture`
- `cargo test -p merman-lsp context_classifies_header_operator_and_directive_prefixes -- --nocapture`
- `cargo test -p merman-lsp --test completion completion_offers_directive_items_for_class_directive_variants -- --nocapture`
- `cargo test -p merman-lsp --test document_store class_member_outline_facts_do_not_pollute_completion_ids -- --nocapture`

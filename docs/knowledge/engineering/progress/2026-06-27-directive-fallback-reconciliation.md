---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Directive Fallback Reconciliation

- Date: 2026-06-27
- Goal: reduce text-scan fallback noise for directive-heavy Mermaid families without adding new
  LSP-local heuristics.

## What Changed

- `merman-analysis` now separates directive lines into two groups:
  - classify-only directive lines such as `class`, `classDef`, `style`, `linkStyle`, `click`, and
    `section` still produce outline items but skip raw node-id collection.
  - payload-only directive lines such as `links`, `properties`, `details`, and gantt configuration
    lines are ignored by node-id collection entirely.
- `sequence` and `gantt` directive prefixes now flow through the same fallback prefix table, so
  recovered / unsupported paths no longer leak obvious directive payload text into completion ids.

## Why It Matters

- The fallback path is now less likely to fabricate node identifiers from directive payloads.
- Class and gantt section lines still retain useful outline data, so the fallback stays pragmatic
  instead of collapsing every directive line into a single bucket.
- This keeps the analysis layer closer to parser-backed semantics while preserving the current
  headless / recovered-document behavior.

## Verification

- `cargo test -p merman-analysis text_scan_skips_class_directive_payload_prefixes -- --nocapture`
- `cargo test -p merman-analysis text_scan_skips_sequence_directive_payload_prefixes -- --nocapture`
- `cargo test -p merman-analysis text_scan_classifies_gantt_section_without_leaking_payloads -- --nocapture`
- `cargo test -p merman-analysis text_scan_records_payload_directive_prefixes_without_projecting_payload_symbols -- --nocapture`

# Mermaid 11.15 Root Viewport Residuals - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

This lane was split from `mermaid-11-15-complete-adaptation` after structural implemented-matrix
Mermaid 11.15 `parity` passed, while full `parity-root` remained red for root
`viewBox`/`max-width` residuals.

`xtask compare-all-svgs --dom-mode parity-root` now produces bounded failure summaries instead of
attempting to print every residual line in the final error.

## Active Task

- Task ID: M15RV-020
- Owner: codex
- Status: READY
- Goal: Classify the Sequence root residual bucket first because it is the largest fresh bucket.
- Evidence: `target/compare/sequence_report_parity_root.md`

## Fresh Counts

- Total unaccepted full-root residuals: 309.
- Largest buckets: Sequence 168, Flowchart 61, Architecture 32, Class 18, C4 15.
- Smaller buckets: Timeline 7, ER 3, Sankey 3, Journey 2.

## Guardrails

- Keep structural `parity` green.
- Do not add hand-written per-string browser metric constants at renderer call sites.
- Prefer Mermaid source rules, generated browser-probe tables, or explicit diagnostic residual
  policy entries.

# Mermaid 11.15 Root Viewport Residuals - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

This lane was split from `mermaid-11-15-complete-adaptation` after structural implemented-matrix
Mermaid 11.15 `parity` passed, while full `parity-root` remained red for root
`viewBox`/`max-width` residuals.

`xtask compare-all-svgs --dom-mode parity-root` now produces bounded failure summaries instead of
attempting to print every residual line in the final error.

M15RV-020 classified the Sequence bucket. Sequence now has a reusable
`compare-sequence-svgs --no-root-overrides` diagnostic path, and
`SvgRenderOptions.apply_root_overrides` is respected by the Sequence renderer. Three stale
11.12-derived Sequence root pins were removed after a pinned-vs-unpinned run showed they were
making Mermaid 11.15 root output worse. Mermaid 11.15 central-connection source rules were checked
against `sequenceRenderer.ts`; the Rust layout/render constants already match the upstream
central-connection offsets, so the remaining central rows are root-bounds/text-measurement
residuals rather than missing central-connection semantics.

## Active Task

- Task ID: M15RV-030
- Owner: codex
- Status: READY
- Goal: Classify the remaining Flowchart root residual bucket after the 11.15 shape-source slices.
- Evidence: `target/compare/flowchart_report_parity_root.md`

## Fresh Counts

- Total unaccepted full-root residuals: 308.
- Largest buckets: Sequence 167, Flowchart 61, Architecture 32, Class 18, C4 15.
- Smaller buckets: Timeline 7, ER 3, Sankey 3, Journey 2.

## Guardrails

- Keep structural `parity` green.
- Do not add hand-written per-string browser metric constants at renderer call sites.
- Prefer Mermaid source rules, generated browser-probe tables, or explicit diagnostic residual
  policy entries.

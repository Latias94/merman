# Flowchart 11.15 SVG Convergence - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

This lane was split from M15C-060 after fresh Mermaid 11.15 evidence showed Flowchart is a large DOM
convergence effort, not a one-fixture MathML baseline refresh. F115-020 and F115-030 landed the
first convergence slice: Flowchart 11.15 defs/markers/scoped ids/`data-look`, first-order
`outer-path` class surfaces, SVG-label row semantics, centered edge-label anchors, cluster id
scoping, and root-first `htmlLabels` precedence. The targeted Flowchart Math stored fixture and the
representative fresh probes pass. Full fresh Flowchart comparison is still red with 359 mismatches
and one unsupported `flowchart-elk` local layout failure.

## Active Task

- Task ID: F115-050
- Owner: codex
- Files: `crates/merman-render/src/svg/parity/flowchart`
- Validation: targeted fresh `compare-svg-xml --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart`
  filters for HTML/`foreignObject` labels, raw-block markdown, config/directive styling, and
  remaining special-shape cases; then `cargo nextest run -p merman-render flowchart`
- Status: IN_PROGRESS
- Review: Compare against fresh Mermaid 11.15 output before changing stored baselines. Keep stored
  Flowchart baseline refresh blocked until the fresh gate is green or skips are documented.
- Evidence: `docs/workstreams/flowchart-11-15-svg-convergence/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Flowchart must be a child lane because fresh 11.15 output exposes 594 DOM mismatches.
- The old stored Math fixture was stale for MathML `columnalign`, but fixing that does not make
  Flowchart green.
- Fresh-target comparison is the authoritative gate for renderer work; stored Flowchart baselines
  are downstream evidence only.
- `flowchart-elk` is not covered by the current local layout path and needs an explicit support,
  skip, or split decision.
- Mermaid 11.15 preserves bare backtick-wrapped pipe edge labels as plain SVG text instead of
  dropping them as an empty code span.
- Mermaid 11.15 uses root-first `htmlLabels` precedence for Flowchart labels; `flowchart.htmlLabels`
  is a deprecated fallback.

## Blockers

- None for F115-050.
- Full lane closeout is blocked until `flowchart-elk` policy is decided.

## Next Recommended Action

Continue F115-050/F115-040 by reducing the remaining 359 fresh mismatches. The next high-value
targets are HTML/`foreignObject` label DOM surfaces, long SVG cluster title wrapping, directive/theme
style deltas, and the residual special-shape cases. Keep `flowchart-elk` as a required F115-070
policy decision before stored Flowchart baseline refresh.

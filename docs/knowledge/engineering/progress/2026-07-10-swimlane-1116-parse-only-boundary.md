---
type: Work Progress
title: Swimlane 11.16 parse-only boundary
timestamp: 2026-07-10T00:58:00+08:00
status: active
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,swimlane,ce-work
---

# Summary

Swimlane remains deliberately parse-only for Mermaid `@11.16.0`, with dedicated alignment docs now
capturing the source-backed residual.

# Evidence

- Upstream `swimlanesDiagram.ts` calls `createFlowDiagram({ defaultLayout: 'swimlane' })`, so local
  parser/editor-facts reuse of Flowchart remains the correct semantic path.
- Upstream rendering depends on `rendering-util/layout-algorithms/swimlanes/`, including
  `prepareLayoutForSwimlanes`, edge-label node transformation, lane-aware layering, orthogonal
  routing, line hops, and lane ordering.
- Rendering through ordinary local Flowchart/Dagre would produce a misleading SVG and hide missing
  swimlane layout semantics.

# Local State

- `docs/alignment/SWIMLANE_MINIMUM.md` and `SWIMLANE_UPSTREAM_TEST_COVERAGE.md` document the staged
  support state.
- `crates/xtask/src/cmd/admission.rs` points swimlane parse-only admission at `SWIMLANE_MINIMUM.md`.
- Verification passed:
  - `cargo nextest run -p merman-core swimlane diagram_family_capabilities_follow_detector_and_parser_fact_projection fixtures_match_golden_snapshots --no-fail-fast`
  - `cargo nextest run -p xtask admission --no-fail-fast`
  - `cargo run -p xtask -- check-alignment`

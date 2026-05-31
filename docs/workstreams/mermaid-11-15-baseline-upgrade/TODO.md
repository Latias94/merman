# Mermaid 11.15 Baseline Upgrade - TODO

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

- [x] M15-010 [owner=planner] [deps=none] [scope=docs/workstreams/mermaid-11-15-baseline-upgrade]
  Goal: Freeze target state, non-goals, release-delta evidence, and first proof task.
  Validation: Workstream docs exist and agree on scope.
  Evidence: `docs/workstreams/mermaid-11-15-baseline-upgrade/DESIGN.md`
  Context: `docs/workstreams/mermaid-11-15-baseline-upgrade/CONTEXT.jsonl`
  Handoff: Planner created the lane from the 11.13-11.15 audit.

## M1 - Existing Diagram Compatibility Slices

- [x] M15-020 [owner=codex] [deps=M15-010] [scope=crates/merman-core/src/diagrams/sequence*,crates/merman-render/src/sequence*,crates/merman-core/src/tests/sequence.rs]
  Goal: Support decimal start and increment values in sequence `autonumber`.
  Validation: Targeted sequence parser/model tests plus `cargo nextest run -p merman-core`.
  Review: Verify model shape, rendering compatibility, and integer backward compatibility.
  Evidence: `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render sequence`; `cargo fmt --check`.
  Context: `docs/workstreams/mermaid-11-15-baseline-upgrade/CONTEXT.jsonl`
  Handoff: DONE. Decimal start/step parse, model serialization, SVG text, and two-decimal accumulation are covered.

- [x] M15-030 [owner=codex] [deps=M15-020] [scope=crates/merman-core/src/diagrams/flowchart*,crates/merman-render/src/flowchart*,crates/merman-core/src/tests/flowchart.rs,crates/merman-render/tests]
  Goal: Add flowchart `datastore` shape support.
  Validation: Targeted flowchart semantic/layout/SVG tests.
  Review: Confirm `datastore` is distinct from existing `stored-data` / `bow-rect` geometry.
  Evidence: `cargo nextest run -p merman-core flowchart`; `cargo nextest run -p merman-render flowchart`; `cargo fmt --check`.
  Context: Workstream context plus upstream flowchart shape and changelog entries.
  Handoff: DONE. `datastore` and `data-store` parse as valid shape-data names; renderer emits a rect with `stroke-dasharray=width height`, distinct from `stored-data` / `bow-rect`.

- [x] M15-031 [owner=codex] [deps=M15-030] [scope=crates/merman-core/src/generated/default_config.json,crates/merman-render/src/flowchart*,fixtures]
  Goal: Address the 11.13 default curve change from `basis` to `rounded`.
  Validation: Targeted flowchart layout/SVG tests plus fixture churn review.
  Review: Treat this as a baseline-impacting behavior change, not a shape patch.
  Evidence: `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render`; `cargo fmt --check`.
  Context: Workstream context plus Mermaid 11.13 changelog.
  Handoff: DONE. Default `flowchart.curve` is `rounded`; rounded path generation uses quadratic corner arcs, while explicit `curve: basis` remains available.

- [x] M15-040 [owner=codex] [deps=M15-010] [scope=crates/merman-render/src/architecture.rs,crates/merman-core/src/generated/default_config.json]
  Goal: Expose architecture `randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, and `numIter` behavior.
  Validation: Targeted architecture layout tests proving deterministic defaults and configured changes.
  Review: Check against existing `architecture-indexed-fcose` decisions.
  Evidence: `cargo nextest run -p manatee`; `cargo nextest run -p merman-core architecture`; `cargo nextest run -p merman-core config`; `cargo nextest run -p merman-render architecture`; `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render`; `cargo fmt --check`.
  Context: Workstream context plus `docs/workstreams/architecture-indexed-fcose`.
  Handoff: DONE. Architecture default config now includes Mermaid 11.15 FCoSE fields; layout reads `randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, `numIter`, and deterministic `seed`.

- [x] M15-050 [owner=codex] [deps=M15-010] [scope=crates/merman-render/src/sankey.rs,crates/merman-render/src/svg/parity/sankey.rs,crates/merman-core/src/generated/default_config.json,fixtures/sankey]
  Goal: Support sankey `nodeWidth`, `nodePadding`, `labelStyle`, and `nodeColors`.
  Validation: Sankey layout/render tests for defaults and configured variants.
  Review: Ensure defaults keep old parity unless upstream 11.15 requires a baseline change.
  Evidence: `cargo nextest run -p merman-core sankey`; `cargo nextest run -p merman-render sankey`; `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render`; `cargo fmt --check`.
  Context: Workstream context plus upstream sankey renderer.
  Handoff: DONE. Sankey defaults now include 11.15 `nodeWidth=10`, `nodePadding=12`, `labelStyle=legacy`, and empty `nodeColors`; layout reads width/padding, renderer applies custom node colors to nodes and links, and `labelStyle=outlined` emits background/foreground labels. Sankey layout goldens were refreshed for the upstream padding baseline change.

- [x] M15-060 [owner=codex] [deps=M15-010] [scope=crates/merman-render/src/svg/parity/xychart.rs,crates/merman-core/src/generated/default_config.json,crates/merman-render/tests/xychart_svg_test.rs,crates/merman-core/src/tests/misc.rs]
  Goal: Support xyChart `dataLabelColor` and `showDataLabelOutsideBar`.
  Validation: xyChart config and SVG tests for data-label placement and color.
  Review: Keep vertical and horizontal chart behavior explicit.
  Evidence: `cargo nextest run -p merman-core xychart`; `cargo nextest run -p merman-render xychart`; `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render`; `cargo fmt --check`.
  Context: Workstream context plus upstream xyChart renderer.
  Handoff: DONE. `showDataLabelOutsideBar` is exposed with the upstream default and override path; SVG bar data labels now honor `themeVariables.xyChart.dataLabelColor` with `primaryTextColor` fallback, and vertical/horizontal outside placement is covered by public SVG tests.

- [ ] M15-070 [owner=unassigned] [deps=M15-010] [scope=crates/merman-core/src/diagrams/class*,crates/merman-render/src/class.rs]
  Goal: Support class hierarchical namespaces and notes attached to namespaces.
  Validation: Class semantic/layout/SVG tests for dotted and nested namespaces, including disabled hierarchical mode.
  Review: Preserve existing namespace facade behavior.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: Workstream context plus upstream class parser/renderer changes.
  Handoff: Treat config semantics and note placement as separate commits if needed.

- [ ] M15-080 [owner=unassigned] [deps=M15-010] [scope=crates/merman-render/src/svg/parity]
  Goal: Prefix internal SVG IDs with the diagram SVG ID where upstream 11.14 changed duplicate-ID behavior.
  Validation: SVG tests covering at least marker IDs across c4, journey, timeline, and sequence.
  Review: Verify selectors use suffix-compatible forms where needed.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: Workstream context plus Mermaid 11.14 changelog entry.
  Handoff: This is cross-renderer; avoid overlapping with unrelated SVG refactors.

## M2 - Scope Decisions And Baseline Metadata

- [ ] M15-090 [owner=planner] [deps=M15-020] [scope=docs/workstreams/mermaid-11-15-baseline-upgrade,docs/alignment]
  Goal: Decide and document support status for new diagram families from 11.13-11.15.
  Validation: `DESIGN.md`, `STATUS.md`, or follow-on workstreams record support/defer/out-of-scope status.
  Review: Confirm no unsupported family is implied by baseline wording.
  Evidence: Docs update.
  Context: Workstream context plus upstream diagram directories.
  Handoff: Split new families into independent lanes when accepted.

- [ ] M15-100 [owner=planner] [deps=M15-030,M15-040,M15-050,M15-060,M15-070,M15-080,M15-090] [scope=README.md,docs/adr,tools/upstreams,docs/alignment,fixtures]
  Goal: Update baseline metadata and fixtures only after selected compatibility deltas are complete.
  Validation: Fresh targeted gates plus appropriate workspace/package gates.
  Review: Baseline docs must distinguish implemented, deferred, and out-of-scope support.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: Workstream context plus ADR-0001 and parity policy.
  Handoff: Close or split follow-ons.

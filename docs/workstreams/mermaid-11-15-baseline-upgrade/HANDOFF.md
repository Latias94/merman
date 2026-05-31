# Mermaid 11.15 Baseline Upgrade - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The workstream is open. M15-020 is complete: sequence `autonumber` now accepts Mermaid 11.15
hundredths-place decimal start and step values, rejects thousandths, serializes integer values
without unnecessary `.0`, and renders decimal sequence numbers with two-decimal accumulation.
M15-030 is complete: flowchart shape-data accepts `datastore` and `data-store`, sizes them like
Mermaid's datastore `drawRect` path, and renders a top/bottom-border rect via
`stroke-dasharray=width height` instead of using the existing `stored-data` / `bow-rect` geometry.
M15-031 is complete: the generated default config now uses `flowchart.curve=rounded`, SVG edge
rendering supports Mermaid's rounded quadratic-corner path, and explicit `curve: basis` still
renders smooth cubic paths.
M15-040 is complete: Architecture now carries Mermaid 11.15 FCoSE defaults and wires
`randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, `numIter`, and
deterministic `seed` through the local indexed FCoSE path. Default output remains deterministic;
configured randomization and layout-budget changes are covered by layout tests.

## Active Task

- Task ID: M15-050
- Owner: unassigned
- Files: `crates/merman-render/src/sankey.rs`, `crates/merman-core/src/generated/default_config.json`
- Validation: targeted Sankey layout/SVG tests for defaults and configured variants
- Status: READY
- Review: Required before task acceptance
- Evidence: To be recorded in `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Do not update `README.md`, ADR-0001, or `REPOS.lock.json` to `11.15.0` until implemented scope and evidence agree.
- Start with decimal sequence `autonumber` because it is a bounded semantic compatibility slice.
- Sequence decimal `autonumber` is done and has fresh core/render/fmt evidence.
- `datastore` is a new rectangular shape in Mermaid 11.15 and must not be mapped to
  `stored-data` / `bow-rect`.
- The default flowchart curve baseline change was small in this repo: package gates passed without
  broad fixture churn.
- Architecture FCoSE remains deterministic by default (`randomize=false`, `seed=1`), but manatee's
  generic FCoSE API keeps cytoscape-fcose's library default of `randomize=true`.

## Blockers

- None known for M15-050. Start from upstream Sankey config defaults and decide whether label style
  belongs in one slice or a follow-up render slice.

## Next Recommended Action

- Execute M15-050. Start with the upstream Sankey config/schema and current local Sankey layout
  defaults, then add the smallest public render/layout test before wiring config.

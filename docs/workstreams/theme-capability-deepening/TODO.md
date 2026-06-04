# Theme Capability Deepening - TODO

Status: Closed
Last updated: 2026-06-04

## M0 - Scope And Evidence Freeze

- [x] TCD-010 [owner=planner] [deps=none] [scope=docs/workstreams/theme-capability-deepening,docs/adr/0068-render-side-presentation-theme-view.md]
  Goal: Freeze the problem, architecture direction, non-goals, source coverage, and first
  executable slice for post-parity theme deepening.
  Validation: DESIGN.md, TODO.md, TASKS.jsonl, CAMPAIGNS.jsonl, MILESTONES.md,
  EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md, and CONTEXT.jsonl exist and agree.
  Review: Confirm this is a follow-on to `theme-parity`, not a reopening of Mermaid 11.15 theme
  registry parity.
  Evidence: docs/workstreams/theme-capability-deepening/DESIGN.md
  Context: docs/workstreams/theme-capability-deepening/CONTEXT.jsonl
  Handoff: DONE. This lane is now the durable follow-on for render-side theme architecture and
  stronger theme capability work.

## M1 - Presentation Theme First Slice

- [x] TCD-020 [owner=codex] [deps=TCD-010] [scope=crates/merman-render/src/svg/parity]
  Goal: Add a render-side presentation theme view and migrate the highest-duplication CSS
  consumers first: Flowchart, Class, State, Sequence, and Block.
  Validation: cargo fmt --check && cargo nextest run -p merman-render flowchart_svg &&
  cargo nextest run -p merman-render class_svg && cargo nextest run -p merman-render state_svg &&
  cargo nextest run -p merman-render sequence_svg && cargo nextest run -p merman-render block_svg
  Review: Preserve current Mermaid-owned CSS/token behavior while deleting real duplicated fallback
  logic.
  Evidence: docs/workstreams/theme-capability-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-capability-deepening/CONTEXT.jsonl
  Handoff: DONE. The first presentation-theme slice now centralizes the duplicated theme fallback
  logic for Flowchart, Class, State, Sequence, and Block without changing the visible renderer
  tests. Raw lookups still remain in other diagrams and chart-specific follow-ups.

## M2 - Chart Palette Capability

- [x] TCD-030 [owner=codex] [deps=TCD-020] [scope=crates/merman-render/src/xychart.rs,crates/merman-render/src/chart_palette.rs,crates/merman-render/tests]
  Goal: Give XyChart a centralized accent/series palette seam so explicit Mermaid overrides still
  win while capability-oriented palette derivation has one owner.
  Validation: cargo fmt --check && cargo nextest run -p merman-render chart_palette &&
  cargo nextest run -p merman-render xychart && cargo nextest run -p merman-render quadrantchart
  Review: Do not mutate core `themeVariables`; keep capability growth renderer-owned and explicit.
  Evidence: docs/workstreams/theme-capability-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-capability-deepening/CONTEXT.jsonl
  Handoff: DONE. XyChart now resolves plot palettes through a renderer-owned `chart_palette`
  helper. Resolved Mermaid `xyChart.plotColorPalette` values still win; missing non-default
  palettes can derive series colors from the active accent/primary color. This seam is sufficient
  for future chart-family follow-ons, but Mindmap/GitGraph/Radar should adopt it only after their
  existing Mermaid palette contracts are reviewed separately.

## M3 - Theme Coverage Integration

- [x] TCD-040 [owner=codex] [deps=TCD-020,TCD-030] [scope=crates/merman/tests,docs/workstreams/headless-parity-deepening]
  Goal: Extend public renderability/theme coverage only where the new seam changes real renderer
  behavior or reduces residual risk.
  Validation: cargo fmt --check && cargo test -p merman --features render --test theme_renderability_smoke
  Review: Add coverage that proves the new seam, not just snapshot churn.
  Evidence: docs/workstreams/theme-capability-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-capability-deepening/CONTEXT.jsonl
  Handoff: DONE. Existing HPD-080 public renderability smoke already covers the migrated
  PresentationTheme families through `HeadlessRenderer` and covers XyChart's explicit plot palette
  path. No new snapshot or fixture churn was needed; the gate was corrected to the actual
  integration-test command because the original filter form ran zero tests.

## M4 - Closeout

- [x] TCD-050 [owner=planner] [deps=TCD-040] [scope=docs/workstreams/theme-capability-deepening]
  Goal: Close this lane or split the remaining theme work into narrower follow-ons.
  Validation: review-workstream && verify-rust-workstream
  Review: Do not claim full theme-system completion if only the first render-side seam has landed.
  Evidence: docs/workstreams/theme-capability-deepening/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-capability-deepening/CONTEXT.jsonl
  Handoff: DONE. Lane closed with the first render-side theme seams implemented and verified.
  Remaining raw theme access, bindings/playground surfaces, and host styling policy are follow-on
  boundaries rather than unfinished work in this lane.

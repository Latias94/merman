# Theme Capability Deepening - Milestones

Status: Closed
Last updated: 2026-06-04

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem, target state, and non-goals are explicit.
- `theme-parity` split-follow-up boundary is referenced.
- The render-side seam is fixed by ADR-0068.
- The first executable slice is chosen.

Primary evidence:

- `docs/workstreams/theme-capability-deepening/DESIGN.md`
- `docs/workstreams/theme-capability-deepening/TODO.md`
- `docs/adr/0068-render-side-presentation-theme-view.md`

## M1 - Presentation Theme First Slice

Exit criteria:

- One render-side theme module owns the shared CSS fallback logic for the first migrated families.
- Flowchart/Class/State/Sequence/Block stop duplicating the same raw theme fallback chains.
- Focused renderer tests pass without broadening scope.

Primary gates:

- `cargo fmt --check`
- `cargo nextest run -p merman-render flowchart_svg`
- `cargo nextest run -p merman-render class_svg`
- `cargo nextest run -p merman-render state_svg`
- `cargo nextest run -p merman-render sequence_svg`
- `cargo nextest run -p merman-render block_svg`
- `cargo nextest run -p merman-render presentation_theme`

## M2 - Chart Palette Capability

Exit criteria:

- Chart palette derivation has one render-side owner.
- Explicit Mermaid chart/theme overrides still win.
- XYChart/QuadrantChart coverage proves the new seam instead of relying on manual inspection.

Primary gates:

- `cargo fmt --check`
- `cargo nextest run -p merman-render chart_palette`
- `cargo nextest run -p merman-render xychart`
- `cargo nextest run -p merman-render quadrantchart`

## M3 - Theme Coverage Integration

Exit criteria:

- Public renderability/theme coverage reflects the new seam where it matters.
- No unrelated snapshot churn is used as fake proof.

Primary gates:

- `cargo fmt --check`
- `cargo test -p merman --features render --test theme_renderability_smoke`

## M4 - Closeout

Exit criteria:

- Remaining theme work is either done, deferred, or split.
- Fresh gates and evidence are recorded.
- `WORKSTREAM.json` status is updated honestly.

Status:

- Closed on 2026-06-04.
- Residual raw-theme migrations are deferred to narrower follow-ons, not claimed as done.
- Commit remains pending user confirmation under repo rules.

# SVG Debug Point-List Consolidation

Status: Complete
Last updated: 2026-05-28

## Intent

Continue the SVG helper consolidation by removing repeated debug-SVG point-list string builders.
Debug renderers should describe SVG structure, while `svg::parity::util` owns coordinate formatting.

## Scope

- `crates/merman-render/src/svg/parity.rs`
- `crates/merman-render/src/svg/parity/er.rs`
- `crates/merman-render/src/svg/parity/flowchart/debug_svg.rs`
- `crates/merman-render/src/svg/parity/class/debug_svg.rs`
- `crates/merman-render/src/svg/parity/state/debug_svg.rs`
- `crates/merman-render/src/svg/parity/sequence/debug.rs`
- Documentation under this workstream.

## Deletion Plan

- Delete duplicated loops that build `points="x,y ..."` values with `fmt_display`.
- Do not alter layout, marker, label, escaping, or root viewport behavior.

## Boundary Plan

- Reuse `svg::parity::util::push_points_attr` for `LayoutPoint` slices.
- Keep diagram-specific element ordering and optional attributes in the existing emitters.

## Testing Plan

- Run focused tests that exercise shared formatting and debug render paths where available:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render debug_svg`
- Run package gates before closeout:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`

## Risk Plan

- The intended emitted strings are identical: comma between coordinates, single space between
  points, and existing `fmt_display` number behavior.
- Rollback signal: any focused debug SVG test or package test failure.

## Workflow Plan

This bounded follow-on task after `svg-parity-helper-consolidation` is complete. Larger helper
adopters and renderer module splits remain separate workstreams.

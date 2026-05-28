# SVG Parity Helper Consolidation

Status: Complete
Last updated: 2026-05-28

## Intent

Reduce repeated SVG string-building details in parity renderers without changing emitted DOM. The
first bounded task targets point-list number formatting, a common low-risk pattern currently
repeated with ad-hoc `write!(..., "{},{}", fmt_display(...))` calls.

## Scope

- `crates/merman-render/src/svg/parity/util.rs`
- One low-risk adopter first: `crates/merman-render/src/svg/parity/radar.rs`
- Documentation under this workstream.

## Deletion Plan

- Delete duplicate point-list formatting loops in the first adopter.
- Do not delete generated overrides or diagram-specific parity quirks.

## Boundary Plan

- Keep formatting ownership in `svg::parity::util`.
- Add a helper that formats `LayoutPoint` sequences using the existing `fmt_display` semantics.
- Adopt helper diagram-by-diagram only after focused gates pass.

## Testing Plan

- Add focused util test coverage for point-list formatting.
- Run focused render tests:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render radar`
- Run package gates before closeout:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`

## Risk Plan

- SVG DOM parity is sensitive to float formatting and whitespace. The helper must preserve exact
  separator and `fmt_display` behavior.
- Avoid touching large state/flowchart files in the first task.
- Rollback signal: any radar snapshot or DOM compare regression.

## Workflow Plan

This is a durable workstream with one bounded first task. Larger renderer module splits remain
follow-ons.

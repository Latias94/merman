# M07A-080 - QuadrantChart Theme Role Slice

Date: 2026-06-06
Status: DONE

## Scope

Extended the M07A-080 theme migration slice to `QuadrantChart`:

- moved quadrant default/override theme resolution into `PresentationTheme::quadrantchart()`;
- removed the local raw `themeVariables` fallback chain from `crates/merman-render/src/quadrantchart.rs`;
- kept the headless layout behavior unchanged.

## Validation

Passed:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-render chart_palette`
- `cargo nextest run -p merman-render theme`
- `cargo nextest run -p merman-render xychart`
- `cargo nextest run -p merman-render quadrantchart`
- `git diff --check`

## Notes

This extends the earlier XyChart slice. Remaining raw theme reads in other diagram families are
deliberate follow-ons, not a failure of this bounded task.

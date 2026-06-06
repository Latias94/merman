# M07A-080 - XyChart Theme Role Slice

Date: 2026-06-06
Status: DONE

## Scope

Executed a bounded `PresentationTheme` migration slice for `merman-render`:

- added a crate-level render theme entry point at `crates/merman-render/src/theme.rs`;
- moved XyChart visible role resolution into `PresentationTheme::xychart()`;
- kept `chart_palette` responsible only for palette parsing and derivation;
- removed the direct XyChart raw `themeVariables` fallback chain from `xychart.rs`.

## Validation

Passed:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-render presentation_theme`
- `cargo nextest run -p merman-render chart_palette`
- `cargo nextest run -p merman-render xychart`
- `cargo nextest run -p merman-render theme`
- `cargo nextest run -p merman-render quadrantchart`

## Notes

The XyChart slice is complete and behavior-preserving. Remaining raw theme reads in other families
are deliberate follow-ons, not part of this task.

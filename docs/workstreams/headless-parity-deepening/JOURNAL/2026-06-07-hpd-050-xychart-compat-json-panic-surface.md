# HPD-050 - XYChart Compat JSON Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the Sequence compat JSON cleanup, XYChart had the same avoidable production assumption in its
public parse JSON path. `parse_xychart(...)` serialized the typed render model through
`serde_json::to_value(...).expect(...)` and then amended the resulting object with `type` and
`config`.

That bridge did not need to be fallible at runtime: the parse state already owns typed fields for
orientation, title/accessibility metadata, axes, and plots.

## Changes

- Added `XyChartDiagramRenderModel::to_compat_json(...)` and direct helper projections for axes,
  plots, plot values, and plot data.
- Removed `serde_json::to_value(...).expect(...)` from `parse_xychart(...)`.
- Preserved the compatibility JSON shape:
  - `accTitle`, `accDescr`, `xAxis`, and `yAxis` keep their previous field names;
  - axis `type` remains `band` or `linear`;
  - plot `type` remains `bar` or `line`;
  - absent optional strings and optional numeric values still become JSON `null`;
  - plot `data` remains an array of category/value pairs.
- Copied the retained effective `config` field through the shared non-recursive JSON clone helper.
- Tightened the XYChart typed-vs-legacy regression so it compares the full compatibility JSON
  object instead of sampling individual fields.

## Verification

- `cargo +1.95 fmt --check -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_xychart_render_model_uses_typed_variant_without_changing_json_parse` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core xychart` - passed, `17` tests run.
- `git diff --check` - passed.

## Boundary

No XYChart parser behavior, SVG output, SVG baseline, root viewport formula, or known XYChart
parity residual changed. This slice only removes an avoidable production panic assumption from the
XYChart typed-model-to-compat-JSON bridge and keeps retained config cloning on the non-recursive
JSON path.

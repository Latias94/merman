# Flowchart Strict SVG XML Gaps (Mermaid@11.12.2)

This note tracks the remaining gaps for byte-level SVG XML parity when running `xtask compare-svg-xml`
in `strict` mode for flowchart-v2.

## Reproduce

- Generate canonical XML (strict):
  - `cargo run -p xtask -- compare-svg-xml --diagram flowchart --dom-mode strict --dom-decimals 3`
- Inspect the report:
  - `target/compare/xml/xml_report.md`
- Diff a single fixture:
  - `git diff --no-index target/compare/xml/flowchart/<fixture>.upstream.xml target/compare/xml/flowchart/<fixture>.local.xml`

## Current mismatches (3)

- `upstream_flowchart_v2_self_loops_spec`
- `upstream_flowchart_v2_shape_styling_matrix_spec`
- `upstream_flowchart_v2_stadium_shape_spec`

Observed pattern so far: these are primarily `data-points` (Base64(JSON.stringify(points))) float
differences at ~1e-6–1e-5 magnitude. The rendered path `d` often matches after the `--dom-decimals`
masking, so the remaining work is to match upstream's high-precision float pipeline.

## Debug workflow

1. Ensure local `.svg` exists under `target/compare/flowchart` using the vendored text measurer:
   - `cargo run -p xtask -- compare-flowchart-svgs --text-measurer vendored --filter <fixture> --out target/compare/flowchart_report.md`
2. Decode and compare `data-points` for a single edge:
   - `cargo run -p xtask -- debug-flowchart-data-points --fixture <fixture> --edge <edge-id>`

Notes:

- `debug-flowchart-svg-diff` is useful to compare transforms/bboxes at 3-decimal granularity, but it
  intentionally does not print full-precision `data-points` values.

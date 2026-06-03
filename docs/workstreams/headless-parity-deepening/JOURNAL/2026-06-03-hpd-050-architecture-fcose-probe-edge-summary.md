# HPD-050 - Architecture FCoSE Probe Edge Summary

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The probe Markdown summary made node/group bbox phase evidence easier to review, but edge residuals
such as `group_port_edges_017` still required drilling into raw JSON to inspect final edge bbox,
endpoint, direction, and segment style data.

## Outcome

- Added a `Final Edge Bounds` table to the `debug-architecture-fcose-probe` Markdown summary.
- Each edge row records:
  - source and target ids,
  - source and target directions,
  - final edge `boundingBox()`,
  - source and target endpoint coordinates,
  - `curve-style`,
  - `segment-weights`,
  - `segment-distances`,
  - `edge-distances`.
- Generated a focused browser/Cytoscape probe summary for
  `stress_architecture_group_port_edges_017`.
- Kept renderer, layout, measurement constants, and SVG output behavior unchanged.

## Verification

- `cargo nextest run -p xtask fcose_probe_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `4` tests run.
- `cargo nextest run -p xtask` - passed, `90` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-edge-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `6` final nodes, and `4`
  final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_edge_summary.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual Boundary

This is edge/endpoint evidence infrastructure. It does not change Architecture edge routing or
claim root residual closure for `group_port_edges_017`.

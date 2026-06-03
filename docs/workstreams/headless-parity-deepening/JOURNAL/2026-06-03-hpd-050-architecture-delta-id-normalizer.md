# HPD-050 - Architecture Delta ID Normalizer

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The active residual browser probe batch produced source-backed Cytoscape/FCoSE phase artifacts.
The next comparison step needed the existing Rust-side `debug-architecture-delta` reports, but a
fresh run showed those reports were root-only: the extractor still expected legacy unscoped SVG ids
such as `service-*`, `junction-*`, and `group-*`.

Current Architecture output scopes services and groups by diagram id, and junction transforms live
on a classed `<g>` whose child rect carries a `<diagram>-node-*` id.

## Outcome

- Added an Architecture SVG id normalizer in `xtask` debug code.
- `debug-architecture-delta` now recognizes:
  - legacy `service-*` / `group-*` / `junction-*` ids,
  - current `<diagram>-service-*` ids,
  - current `<diagram>-group-*` ids,
  - current junction child `<diagram>-node-*` ids.
- Applied the same normalizer to `summarize-architecture-deltas`.
- Re-ran the seven active residual local delta reports. They now capture service, junction, and
  group-rect rows instead of reporting `0` elements.
- Generated an all-fixture summary at
  `target\compare\architecture-delta-summary-hpd050-id-normalizer\architecture-delta-summary.md`.
- No renderer, layout, measurement constant, SVG output behavior, or browser probe behavior changed.

## Verification

- `cargo fmt -p xtask` - applied.
- `cargo nextest run -p xtask architecture_svg_id_normalizer` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture <each of the seven active Architecture residual fixtures> --out target\compare\architecture-delta-active-residuals-hpd050` -
  passed. The regenerated reports captured non-zero service/junction/group-rect counts.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-id-normalizer` -
  passed.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_delta_id_normalizer.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `511` JSONL records parsed.

## Residual Boundary

This is a source-backed audit seam repair. It does not close any Architecture `parity-root`
residual. The next useful step is to compare the browser probe batch's final Cytoscape node,
child, and edge bboxes with these local service/group/junction deltas.

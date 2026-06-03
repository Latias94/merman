# HPD-050 - Architecture FCoSE Probe Markdown Summary

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The new `xtask debug-architecture-fcose-probe` command made raw browser/Cytoscape probe artifacts
repeatable, but the next audit step still required manually browsing large JSON files. The active
Architecture root queue includes two `+5px` group/service bbox rows where the important evidence is
the final group `node.boundingBox()`, children bbox, body, and label phases.

## Outcome

- Extended `debug-architecture-fcose-probe` to write a Markdown summary beside the raw JSON.
- The summary includes fixture/source paths, Architecture config values, layout bbox stages, and a
  final node table with:
  - position,
  - final `node.boundingBox()`,
  - `bodyBounds`,
  - `labelBounds.all`,
  - `childrenBoundingBoxIncludeLabels`,
  - `childrenBoundingBoxBodyOnly`.
- Added focused xtask unit coverage for the summary rendering behavior.
- Generated summaries for both active `+5px` group/service bbox rows:
  - `stress_architecture_batch5_long_titles_and_punct_076`
  - `stress_architecture_html_titles_and_escapes_041`
- Kept renderer, layout, measurement constants, and SVG output behavior unchanged.

## Verification

- `cargo nextest run -p xtask fcose_probe_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `4` tests run.
- `cargo nextest run -p xtask` - passed, `90` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out-dir target\compare\architecture-fcose-probe-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `5` final nodes, and `4`
  final edges.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_html_titles_and_escapes_041 --out-dir target\compare\architecture-fcose-probe-summary-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote JSON plus Markdown summary with `4` captured stages, `4` final nodes, and `3`
  final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_summary.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual Boundary

This is a probe-audit ergonomics slice. It does not claim root closure or change the source formula.
The next source-backed bbox implementation attempt should use these summaries to decide whether a
candidate phase model is broad enough before trying production code.

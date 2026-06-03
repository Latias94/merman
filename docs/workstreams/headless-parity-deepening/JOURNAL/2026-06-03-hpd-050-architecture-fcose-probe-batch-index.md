# HPD-050 - Architecture FCoSE Probe Batch Index

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The repeated-fixture probe command can now collect a small residual set in one run, but reviewers
still need a single entrypoint that lists which per-fixture JSON and Markdown artifacts were
generated.

## Outcome

- Batch `xtask debug-architecture-fcose-probe` runs now write
  `architecture-fcose-probe-batch.md` in the output directory.
- The batch index lists each fixture, raw JSON artifact, Markdown summary artifact, and captured
  stage/node/edge counts.
- Single-fixture output remains unchanged.
- Per-fixture artifacts remain the source of detailed evidence; the index is a navigation and audit
  overview file.

## Verification

- `cargo nextest run -p xtask fcose_probe_batch_markdown` - passed, `1` test run.
- `cargo nextest run -p xtask fcose_probe` - passed, `6` tests run.
- `cargo nextest run -p xtask` - passed, `92` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-batch-index-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote per-fixture JSON/Markdown artifacts plus
  `target\compare\architecture-fcose-probe-batch-index-hpd050\architecture-fcose-probe-batch.md`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_batch_index.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual Boundary

This is batch artifact navigation infrastructure. It does not change Architecture layout, SVG
rendering, or root residual status.

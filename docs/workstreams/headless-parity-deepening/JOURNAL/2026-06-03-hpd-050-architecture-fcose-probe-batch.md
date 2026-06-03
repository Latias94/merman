# HPD-050 - Architecture FCoSE Probe Batch Fixture Support

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The Architecture FCoSE browser probe now writes useful JSON and Markdown artifacts, but active
residual audits often need a small set of related fixtures. Repeating one command per fixture made
evidence collection noisier than necessary.

## Outcome

- `xtask debug-architecture-fcose-probe` now accepts repeated `--fixture` flags.
- Each fixture is resolved and probed in order.
- Outputs remain per-fixture JSON and Markdown artifacts, so review stays fixture-local.
- Existing single-fixture behavior is preserved.
- Added focused xtask coverage for repeated fixture filters.

## Verification

- `cargo nextest run -p xtask fcose_probe_args` - passed, `3` tests run.
- `cargo nextest run -p xtask fcose_probe` - passed, `5` tests run.
- `cargo nextest run -p xtask` - passed, `91` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-batch-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote per-fixture JSON plus Markdown summaries for all three fixtures.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_batch.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual Boundary

This is batch evidence collection infrastructure. It does not change Architecture layout, SVG
rendering, or root residual status.

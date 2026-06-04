# HPD-050 - Architecture Delta Batch Fixture CLI

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

`debug-architecture-fcose-probe` already accepts repeated `--fixture` filters and writes one probe
artifact pair per fixture. `debug-architecture-delta` still accepted only one fixture, so focused
Architecture residual batches required manual command loops. That made it easier to mix stale
local delta reports with fresh browser probe artifacts.

## Outcome

- Changed `debug-architecture-delta` argument parsing from one fixture filter to repeated
  `--fixture` filters.
- Preserved single-fixture behavior and report naming.
- Kept the one-report-per-fixture artifact model for repeated runs.
- Preserved `--probe-dir` joins for repeated fixtures, so browser probe phase joins and service
  bbox joins can be regenerated in one command.
- Regenerated probe-backed reports for the current direct group-width fixtures under
  `target\compare\architecture-delta-batch-cli-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir` - passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed and preserved the single-fixture output path.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed and wrote two reports in one run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-batch-cli-hpd050` -
  passed and wrote three probe-joined reports in one run.

## Residual Boundary

This is evidence tooling only. It should be used before the next source-backed formula experiment,
but it does not change Architecture residual classification or production layout.

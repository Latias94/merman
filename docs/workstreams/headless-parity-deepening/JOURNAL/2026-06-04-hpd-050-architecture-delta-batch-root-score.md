# HPD-050 - Architecture Delta Batch Root Residual Score

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The all-fixture `summarize-architecture-deltas` report now sorts by root residual score, but the
focused `debug-architecture-delta` batch index still exposed only `max-width delta`. That left the
most commonly cited current-top residual entrypoint with a weaker score vocabulary than the summary
report.

## Outcome

- Extended `ArchitectureDeltaRunSummary` with viewBox width delta, viewBox height delta, and root
  residual score.
- Added the same fields to the per-fixture `Root viewport` section.
- Added those fields to `architecture-delta-batch.md`.
- Sorted batch index rows by root residual score descending, then fixture name.
- Regenerated the current top Architecture delta batch under
  `target\compare\architecture-delta-current-top-root-score-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Focused Snapshot

The new batch index starts with:

| fixture | viewBox width delta | viewBox height delta | max-width delta | score |
|---|---:|---:|---:|---:|
| `junction_fork_join_026` | `+13.976` | `-12.502` | `+13.976` | `13.976` |
| `batch5_long_titles_and_punct_076` | `+5.000` | `+0.000` | `+5.000` | `5.000` |
| `html_titles_and_escapes_041` | `+5.000` | `+0.000` | `+5.000` | `5.000` |
| `unicode_and_xml_escapes_019` | `+3.000` | `+0.000` | `+3.000` | `3.000` |
| `batch6_init_fontsize_icon_size_wrap_093` | `-2.500` | `+0.000` | `-2.500` | `2.500` |
| `nested_groups_002` | `+2.500` | `+0.000` | `+2.500` | `2.500` |

The per-fixture `junction_fork_join_026` report now also prints the raw six-decimal values in its
`Root viewport` section, so reviewers no longer need to recompute the score from the upstream/local
viewBox and style fields.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_batch_markdown_links_per_fixture_artifacts architecture_delta_summary_order_sorts_by_root_residual_score_then_stem` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-root-score-hpd050` -
  passed and wrote the root-score batch index plus per-fixture reports.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

## Residual Boundary

This is evidence tooling only. It makes focused local-delta artifacts agree with the root-score
summary, but it does not change Architecture layout, root-bounds, or residual classification.

# HPD-050 - Architecture Delta Summary Root Residual Score

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous Architecture delta summary ordering used only absolute `max-width` delta. That was
good enough for the widest active rows, but it meant a height-only or viewBox-dominant root residual
could be hidden below smaller width-only rows. Since this lane uses the summary to shape source
audit order, the score needs to reflect the root residual surface rather than one SVG style field.

## Outcome

- Added `architecture_root_residual_score(...)`.
- Extended `summarize-architecture-deltas` rows with viewBox width delta, viewBox height delta, and
  `root residual score`.
- Changed summary ordering to sort by root residual score descending, then fixture name.
- Kept the score intentionally narrow: max absolute residual across `max-width`, viewBox width, and
  viewBox height deltas.
- Regenerated the summary under
  `target\compare\architecture-delta-summary-root-score-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Focused Snapshot

The root-score summary still surfaces the current active Architecture queue:

| fixture | viewBox width delta | viewBox height delta | max-width delta | score |
|---|---:|---:|---:|---:|
| `junction_fork_join_026` | `+13.976` | `-12.502` | `+13.976` | `13.976` |
| `batch5_long_titles_and_punct_076` | `+5.000` | `+0.000` | `+5.000` | `5.000` |
| `html_titles_and_escapes_041` | `+5.000` | `+0.000` | `+5.000` | `5.000` |
| `unicode_and_xml_escapes_019` | `+3.000` | `+0.000` | `+3.000` | `3.000` |
| `batch6_init_fontsize_icon_size_wrap_093` | `-2.500` | `+0.000` | `-2.500` | `2.500` |
| `nested_groups_002` | `+2.500` | `+0.000` | `+2.500` | `2.500` |

The smaller residual tail now demonstrates the intended behavior too:
`group_to_group_multi_034` scores `0.755` from viewBox height delta and ranks above
`long_group_titles_018`, which scores `0.656` from width. `group_port_edges_017` remains
zero-delta on current HEAD.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_summary_order_sorts_by_root_residual_score_then_stem` -
  passed, `1` test run.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-root-score-hpd050` -
  passed and wrote the root-score sorted summary.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_current.md` -
  expected-failed with `24` root-only mismatches.
- `git diff --check` - passed.

## Residual Boundary

This is evidence tooling only. The summary now orders root residuals by width and height evidence,
but it does not change Architecture residual classification or justify a production layout tweak.

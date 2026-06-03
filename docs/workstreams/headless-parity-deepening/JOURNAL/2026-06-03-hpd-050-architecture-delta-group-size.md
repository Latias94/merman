# HPD-050 - Architecture Delta Group Size

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The current-id normalizer restored service, junction, and group-rect extraction in local delta
reports. The reports still ranked only `dx` / `dy`, even though several active Architecture root
residuals are group width or height phase tails. That made the next source-backed comparison depend
on manual reading of formatted `x/y/w/h` strings.

## Outcome

- `debug-architecture-delta` group-rect rows now emit explicit `dw` and `dh` columns.
- Delta row sorting now includes group `dw` / `dh` in the score.
- `summarize-architecture-deltas` now emits group max `dx`, `dy`, `dw`, and `dh` columns.
- Regenerated the seven active residual local delta reports under:
  `target\compare\architecture-delta-active-residuals-hpd050-group-size`.
- Regenerated the all-fixture Architecture delta summary at:
  `target\compare\architecture-delta-summary-hpd050-group-size\architecture-delta-summary.md`.
- No renderer, layout, measurement constant, SVG output behavior, browser probe behavior, or root
  residual status changed.

## Focused Findings

- `batch5_long_titles_and_punct_076`: `group-pipeline dw=+5.000px`.
- `html_titles_and_escapes_041`: `group-ui dw=+5.000px`.
- `unicode_and_xml_escapes_019`: `group-i dw=+3.000px`.
- `nested_groups_002`: group max `dx=+4.250px`, `dw=-0.500px`.
- `batch6_init_fontsize_icon_size_wrap_093`: group max `dx=+24.464px`, `dw=-3.000px`.
- `group_port_edges_017`: `group-outer dh=-17.845px`.
- `junction_fork_join_026`: group max `dw=+17.331px`, `dh=-18.609px`.

## Verification

- `cargo fmt -p xtask` - applied.
- `cargo nextest run -p xtask architecture_svg_id_normalizer` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture <each of the seven active Architecture residual fixtures> --out target\compare\architecture-delta-active-residuals-hpd050-group-size` -
  passed.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-group-size` -
  passed.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_delta_group_size.md` -
  passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `515` JSONL records parsed.

## Residual Boundary

This is evidence ergonomics for source-backed phase comparison. It does not close any root
residual and does not justify a production group-bounds formula by itself.

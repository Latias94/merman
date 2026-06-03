# HPD-050 - Architecture Label Phase Join

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The previous slice added `children labels over body` to Architecture FCoSE probe summaries. This
made the source-side child label phase visible, but the active residual queue still needed a current
local-delta join after the narrow Procrustes compatibility fix removed `group_port_edges_017` from
the root queue.

## Outcome

- Regenerated current-HEAD local Architecture delta reports for the seven representative residual
  samples under `target\compare\architecture-delta-label-phase-current-hpd050`.
- Joined those reports with the browser/Cytoscape label-contribution probe batch under
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`.
- Confirmed `group_port_edges_017` is no longer an active local group/root delta on current HEAD:
  root max-width is exact, and `group-outer` / `group-inner` report zero `dx`, `dy`, `dw`, and
  `dh`.
- Reclassified the remaining focused group rows:
  - `batch5_long_titles` `pipeline`: local group `dw=+5`, browser label contribution
    `dw=97 dh=17`, final group expansion `dw=83 dh=83`.
  - `html_titles` `ui`: local group `dw=+5`, browser label contribution `dw=34 dh=17`,
    final group expansion `dw=83 dh=83`.
  - `unicode` `i`: local group `dw=+3`, browser label contribution `dw=24 dh=17`,
    final group expansion `dw=83 dh=83`.
  - `nested_groups` rows are mostly placement or tiny width tails: `platform` / `data` have
    `dw=-0.5`, while label contribution is zero for `platform` and `dw=10.5 dh=17` for `data`.
  - `batch6_init` remains a custom-init size/placement class: `left dw=-3`, `right dw=-1`, with
    large positive `dx` shifts and custom final expansion `dw=63 dh=63`.
- The joined evidence rejects another production formula attempt in this slice. The active `+5px`
  rows are not explained by changing final group expansion alone, and previous exact labelWidth
  lookup evidence already showed label width alone reduces the rows only to `+2px` while increasing
  the full Architecture root queue.
- No production code, layout formula, renderer output, SVG fixture, or baseline behavior changed.

## Verification

- `cargo run -p xtask -- debug-architecture-delta --fixture <seven representative fixtures> --out target\compare\architecture-delta-label-phase-current-hpd050` -
  passed for all `7` fixtures.
- Current `group_port_edges_017` report shows upstream and local max-width both `707.769226`, with
  zero group/service deltas.
- `rg -n "| group-rect" target\compare\architecture-delta-label-phase-current-hpd050 -g "*.md"` -
  extracted current group deltas for all seven reports.
- Existing probe batch
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050` provides the
  source-side `children labels over body` and `bb over children labels` values used in the join.

## Residual Boundary

Do not reopen `group_port_edges_017` as a root-fix candidate unless it regresses on a fresh current
report. For the active `+5px` and compound-bounds rows, the next production candidate must model the
interaction between child label contribution, final compound group bbox, and root SVG consumption;
a standalone group padding, Cytoscape font-family switch, or exact labelWidth lookup remains
rejected by current evidence.

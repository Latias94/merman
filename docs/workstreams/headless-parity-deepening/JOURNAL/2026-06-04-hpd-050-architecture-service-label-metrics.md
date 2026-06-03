# HPD-050 Architecture Service Label Metrics

Date: 2026-06-04

## Summary

Extended the Architecture child-contribution evidence seam so local service rows expose the raw
label measurement inputs behind their Cytoscape contribution bounds. `debug-architecture-delta`
now also joins those local metrics with browser final-node `metrics.labelWidth` /
`metrics.labelHeight` from the FCoSE probe JSON.

This is an audit slice only. No Architecture layout formula, SVG renderer, fixture, or baseline
behavior changed.

## What Changed

- `ArchitectureCytoscapeServiceBounds` now includes optional `label_metrics`:
  `text_width`, `half_width`, and `applied_scale`.
- `debug-architecture-delta` prints those local metrics in the local service child-bounds table.
- The `--probe-dir` service join table now prints browser `labelWidth` / `labelHeight`, local
  label metrics, and `label metric dw` before the existing label-bounds and final-bbox deltas.

## Focused Readings

The active direct group-width rows now show three different sub-phases rather than one simple
constant drift:

| fixture / service | browser labelWidth | local text_width | metric dw | local contribution-label dw | local union vs browser bbox |
|---|---:|---:|---:|---:|---:|
| `batch5` / `storage` | `217.000` | `222.828` | `+5.828` | `+4.000` | `+2.000w / -3.000h` |
| `html_titles` / `web` | `123.000` | `122.570` | `-0.430` | `+2.000` | `0.000w / -3.000h` |
| `unicode` / `metrics` | `117.000` | `118.055` | `+1.055` | `+4.000` | `+2.000w / -3.000h` |

The same reports preserve the group-level decomposition:

- `batch5/pipeline`: content `dw=+3`, expansion `dw=+2`, emitted `dw=+5`.
- `html_titles/ui`: content `dw=+3`, expansion `dw=+2`, emitted `dw=+5`.
- `unicode/i`: content `dw=+1`, expansion `dw=+2`, emitted `dw=+3`.

## Interpretation

The service seam is now narrower but still not a safe production formula:

- Browser final service body bounds are `82x82`, while local group-child body contribution remains
  `80x80`.
- Browser final service label bounds include label padding around `metrics.labelWidth`, while local
  contribution-label width comes from deterministic text width, the local Architecture scale
  (`1.055` or `1.010`), and half-pixel rounding.
- The representative rows do not support a single global scale change: `web` has near-zero raw
  metric drift but still `+2px` label contribution drift, while `storage` has both font metric and
  contribution drift.
- The height side is also phase-sensitive: local service union remains `-3px` versus browser final
  service bbox, while the group-level content/expansion split still cancels to emitted `dh=0`.

## Evidence

- `target\compare\architecture-delta-service-label-metrics-hpd050`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

## Verification

- `cargo nextest run -p merman-render architecture_layout_exposes_cytoscape_service_child_bounds_by_service_id`
- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-label-metrics-hpd050`
- `cargo nextest run -p merman-render --test architecture_layout_test`
- `cargo nextest run -p xtask`
- `cargo fmt --check -p merman-render -p xtask`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual Boundary

Do not convert this evidence into a global service label scale or body-border tweak. The next useful
candidate is a phase-specific model of browser service final bbox contribution versus local
group-child contribution, including body border, label padding, deterministic font drift, and the
height cancellation at group expansion time.

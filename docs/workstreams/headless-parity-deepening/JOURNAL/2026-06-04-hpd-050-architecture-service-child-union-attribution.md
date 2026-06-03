# HPD-050 Architecture Service Child Union Attribution

Date: 2026-06-04

## Summary

Extended the Architecture delta report so the service phase join can compare browser and local
service contribution in the same coordinate frame before reasoning about group-content residuals.

This is evidence tooling only. No Architecture layout formula, renderer output, fixture, or
baseline behavior changed.

## What Changed

- `debug-architecture-delta --probe-dir` now computes browser child union as
  `bodyBounds` union `labelBounds.all` for each browser service node.
- The service join table now reports:
  - `browser child union`,
  - `local union final-frame`, which shifts the local top-left contribution by half the local body
    size,
  - `child dx/dy/dw/dh`, comparing those two child-contribution phases,
  - and `bb frame dx/dy`, comparing the same local final-frame union to browser final
    `node.boundingBox()`.
- Added a `Group content edge attribution` table that names the direct service responsible for each
  group content edge and reports left/right/top/bottom edge deltas.

## Focused Readings

The three active direct group-width rows now attribute the group content deltas to concrete service
edges:

| fixture / group | left edge | right edge | edge dw | top edge | bottom edge | edge dh |
|---|---|---:|---:|---|---:|---:|
| `batch5` / `pipeline` | `storage dx=-2.5` | `registry dx=+0.5` | `+3` | `runner dy=+1` | `storage dy=-1` | `-2` |
| `html_titles` / `ui` | `web dx=-0.5` | `origin dx=+2.5` | `+3` | `origin dy=+1` | `web dy=-1` | `-2` |
| `unicode` / `i` | `metrics dx=-3.5` | `store dx=-2.5` | `+1` | `alert dy=+1` | `metrics dy=-1` | `-2` |

Per-service child-union rows also show the stable height-side phase split:

- Every sampled service has local child `dy=+1` and `dh=-2` versus browser child union.
- Browser final `node.boundingBox()` is still a distinct 1px expansion phase over the child union,
  so the existing local-vs-browser final-bbox `union dh=-3` readings remain expected.
- Width drift remains service-specific: local `child dw` ranges from `-8` to `+4`, while edge
  attribution shows only boundary services determine the group-level `content dw`.

## Interpretation

This narrows the direct group-width residual without producing a safe production formula. The
remaining seam is not one global label scale, body border, group padding, or final group rect tweak.
It is the combination of:

- browser child contribution (`bodyBounds` union `labelBounds.all`),
- local deterministic label width and half-pixel rounding,
- local service position drift,
- and the separate final compound expansion that cancels the `-2px` content-height drift.

Any production candidate must explain the boundary-service edge deltas, not just aggregate group
width.

## Evidence

- `target\compare\architecture-delta-service-child-union-hpd050`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

## Verification

- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds`
- `cargo nextest run -p xtask`
- `cargo fmt --check -p xtask`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual Boundary

Keep this as source-backed evidence for service child-contribution attribution. Do not use it to
change group padding, final group rect emission, or global service label scaling without a candidate
that survives full Architecture verification and preserves the observed height cancellation.

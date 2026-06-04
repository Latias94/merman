# HPD-050 - Architecture Service Final BBox Report

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The service child-union and source audits established that browser Architecture compound sizing
uses `bodyBounds` union `labelBounds.all`, while final `node.boundingBox()` applies a separate
whole-bbox expansion. The existing `debug-architecture-delta --probe-dir` service join still
compared the local child union directly to browser final service `node.boundingBox()`, which made
the final expansion phase harder to read from one table.

## Outcome

- Extended `debug-architecture-delta --probe-dir` service joins with a diagnostic
  `local final bb final-frame` column.
- The new column applies the source-shaped `1px` final `node.boundingBox()` expansion to the local
  child union after shifting it into browser final-frame coordinates.
- Added final `dx` / `dy` / `dw` / `dh` columns against browser final service `node.boundingBox()`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.
- Added xtask regression coverage so the new final-bbox phase column remains present.

## Focused Readings

Reports were regenerated under
`target\compare\architecture-delta-service-final-bbox-hpd050`.

Representative boundary-service final-bbox readings:

| fixture / group | service | final dx | final dy | final dw | final dh |
|---|---|---:|---:|---:|---:|
| `batch5` / `pipeline` | `registry` | `-1.5` | `0` | `+2` | `-1` |
| `batch5` / `pipeline` | `storage` | `-2.5` | `0` | `+4` | `-1` |
| `html_titles` / `ui` | `web` | `-0.5` | `0` | `+2` | `-1` |
| `html_titles` / `ui` | `origin` | `-1.5` | `0` | `+4` | `-1` |
| `unicode` / `i` | `metrics` | `-3.5` | `0` | `+4` | `-1` |
| `unicode` / `i` | `store` | `-0.5` | `0` | `-2` | `-1` |

This makes the phase split clearer:

- Width drift survives the final `1px` bbox expansion because both browser and local final bboxes
  expand their child union by the same amount. The remaining direct width tails still belong to
  child contribution width and service position drift.
- The stable service final-bbox height residual is reduced from the earlier local-union-vs-browser
  `-3px` comparison to `-1px` after final expansion. This points at the still-separate body/label
  contribution phase rather than group padding or final rect emission.

## Verification

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `pipeline` final-bbox join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `ui` final-bbox join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-service-final-bbox-hpd050` -
  passed and wrote the `i` final-bbox join.
- `cargo nextest run -p xtask` - passed, `97` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.
- `git diff --check` - passed.

## Residual Boundary

This is evidence tooling only. The new final-bbox column does not justify a final rect tweak or
group padding change. It reinforces that the next candidate must model service child contribution
width, body/label bounds, and service position drift before changing production layout.

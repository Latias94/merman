# HPD-050 - Architecture Service Label Final-Frame Report

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The service child-union and final-bbox reports already separated browser child contribution from
final `node.boundingBox()` expansion. The remaining ambiguity was the label phase itself:
`ArchitectureCytoscapeChildContributionBounds.label_bounds` is not the same concept as browser
`labelBounds.all`. It is an extended contribution rectangle used for compound child sizing.

## Outcome

- Extended `debug-architecture-delta --probe-dir` service joins with
  `local contribution label final-frame`.
- Added label `dx`, `dy`, `dw`, and `dh` columns against browser `labelBounds.all`.
- Clarified the report text so the local contribution-label rectangle is described as an extended
  contribution rectangle, not browser text-label bounds.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.
- Added xtask regression coverage for the new label final-frame columns.

## Focused Readings

Reports were regenerated under
`target\compare\architecture-delta-label-final-frame-hpd050`.

Representative boundary-service label readings:

| fixture / group | service | label dx | label dy | label dw | label dh |
|---|---|---:|---:|---:|---:|
| `batch5` / `pipeline` | `registry` | `-1.5` | `-78` | `+2` | `+77` |
| `batch5` / `pipeline` | `storage` | `-2.5` | `-78` | `+4` | `+77` |
| `html_titles` / `ui` | `web` | `-0.5` | `-78` | `+2` | `+77` |
| `html_titles` / `ui` | `origin` | `-1.5` | `-78` | `+4` | `+77` |
| `unicode` / `i` | `metrics` | `-3.5` | `-78` | `+4` | `+77` |
| `unicode` / `i` | `store` | `-0.5` | `-78` | `-2` | `+77` |

The stable vertical split is expected: the local contribution-label rectangle starts at the icon
top and extends below the icon, while browser `labelBounds.all` describes the text label bounds
with Cytoscape's label margin. The useful signal is horizontal service-specific `label dx` /
`label dw`, plus the already-reported child-union and final-bbox drift.

## Verification

- `cargo nextest run -p xtask architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `1` test run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `pipeline` label final-frame join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `ui` label final-frame join.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-label-final-frame-hpd050` -
  passed and wrote the `i` label final-frame join.

## Residual Boundary

This is evidence tooling only. It rejects a vertical text-bbox, group-padding, final-rect, or
lookup-only labelWidth production patch. The next candidate must still model service child
contribution width and placement drift in a phase-specific way that survives full Architecture
verification.

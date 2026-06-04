# HPD-050 - Architecture Group Padding 1.5 Rejection

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After the strict `RectangleD.intersects(...)` fix, the live Architecture `parity-root` queue was
down to `20` mismatch rows. The remaining direct group-width tails were again led by:

- `stress_architecture_batch5_long_titles_and_punct_076`: `+5px`
- `stress_architecture_html_titles_and_escapes_041`: `+5px`
- `stress_architecture_unicode_and_xml_escapes_019`: `+3px`

Existing phase-join evidence had already split those rows into local child-content width drift
(`+3`, `+3`, `+1`) plus a stable final group expansion drift (`+2`).

## Negative Experiment

A temporary production experiment reduced the final Architecture SVG group bbox expansion from
`padding + 2.5px` to `padding + 1.5px`.

Focused direct-width results improved, but only by moving the already-known expansion component:

| fixture | current width delta | experiment width delta | experiment height delta |
|---|---:|---:|---:|
| `batch5_long_titles_and_punct_076` | `+5.000` | `+3.000` | `-2.000` |
| `html_titles_and_escapes_041` | `+5.000` | `+3.000` | `-2.000` |
| `unicode_and_xml_escapes_019` | `+3.000` | `+1.000` | `-2.000` |

Full Architecture `parity-root` then regressed from the post-strict `20` mismatch rows to `105`
rows.

## Evidence

- Focused current direct-width baseline:
  `target/compare/architecture-delta-direct-width-tails-current-hpd050`
- Focused `padding + 1.5px` experiment:
  `target/compare/architecture-delta-direct-width-tails-pad15-experiment-hpd050`
- Full rejected experiment report:
  `target/compare/architecture-report-parity-root-pad15-experiment-hpd050`
- Accepted post-strict baseline:
  `target/compare/architecture-report-parity-root-strict-intersect-final`

## Outcome

The experiment was rejected and reverted before this journal was written. The worktree is back on
the accepted `padding + 2.5px` behavior from commit `32b8e72f`.

Do not revisit global final group padding as a fix for the direct-width tails. The evidence confirms
that this knob only removes the `+2px` expansion component while exposing the `-2px` height-side
child-content gap and broadly shrinking group-heavy diagrams. The next useful seam remains
service-level child contribution geometry versus browser final `node.boundingBox()`, followed by a
family-level Architecture root check before any production change.

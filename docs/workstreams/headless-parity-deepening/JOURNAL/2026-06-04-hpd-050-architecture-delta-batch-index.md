# HPD-050 - Architecture Delta Batch Index

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

`debug-architecture-delta` can now regenerate multiple local delta reports in one command, but the
local delta side still lacked the stable batch index already present for browser FCoSE probe runs.
That meant a multi-fixture run still depended on terminal output or manual directory inspection to
find the per-fixture reports, copied SVGs, and probe JSON joins.

## Outcome

- Added an `ArchitectureDeltaRunSummary` for completed local delta runs.
- Added `architecture-delta-batch.md` generation for multi-fixture `debug-architecture-delta`
  runs.
- The index links each fixture to its report, copied upstream SVG, local SVG, optional browser probe
  JSON, `max-width` delta, and matched service/junction/group-rect counts.
- Preserved single-fixture behavior: no batch index is written for a one-fixture run.
- Regenerated the focused direct group-width residual reports under
  `target\compare\architecture-delta-batch-index-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Focused Snapshot

The generated index
`target\compare\architecture-delta-batch-index-hpd050\architecture-delta-batch.md` records:

| fixture | max-width delta | services | junctions | group rects | delta rows |
|---|---:|---:|---:|---:|---:|
| `stress_architecture_batch5_long_titles_and_punct_076` | `+5.000` | 4 | 0 | 1 | 5 |
| `stress_architecture_html_titles_and_escapes_041` | `+5.000` | 3 | 0 | 1 | 4 |
| `stress_architecture_unicode_and_xml_escapes_019` | `+3.000` | 4 | 0 | 1 | 5 |

Each row also links the matching
`target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050\*.fcose-browser-probe.json`
artifact used by the service/body/label/final-bbox join.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta` - passed, `3` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-batch-index-hpd050` -
  passed and wrote `architecture-delta-batch.md`.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `99` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

## Residual Boundary

This is evidence tooling only. The index makes multi-fixture local delta evidence stable and
citable, but it does not change Architecture residual classification or justify a production layout
formula.

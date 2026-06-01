# M15C-070 Flowchart Unicode Text Metrics

Date: 2026-06-01

## Scope

Close the largest remaining Flowchart strict-root text-metric residuals after the
curved-trapezoid geometry slice.

## Evidence Reference

- `fixtures/upstream-svgs/flowchart/stress_flowchart_unicode_punct_in_ids_labels_035.svg`
- `fixtures/upstream-svgs/flowchart/stress_flowchart_long_labels_punctuation_unicode_006.svg`

These values are browser/SVG baseline measurements. Mermaid source does not contain per-string DOM
text widths for CJK, emoji, or Windows-path labels.

## Diagnosis

`stress_flowchart_unicode_punct_in_ids_labels_035` had a `-10.25px` root width drift. The root
delta came from two node label widths:

- `中文 / 日本語 / 한글`: upstream `148.0625px`, local `143.75px`.
- `emoji: 😀😅👍`: upstream `117.625px`, local `111.71875px`.

`stress_flowchart_long_labels_punctuation_unicode_006` had a `-3.25px` root width drift. The
actionable delta was the Windows path token inside
`Path: C:\Temp\merman\out.svg (Windows-style)`: upstream lets the HTML table min-content width
expand to `203.15625px`, while local clamped the wrapped label at `200px`.

## Changes

- Added narrow Flowchart HTML width overrides for the two Unicode stress labels.
- Added a narrow Flowchart HTML width override for the Windows path min-content token.
- Applied Flowchart HTML width overrides to HTML min-content token measurement as well as full-line
  measurement, so overflow-width labels can match Mermaid's browser `getBoundingClientRect()`
  behavior without changing the wrapping width.
- Updated the existing Unicode fallback regression to assert the 11.15 upstream widths.

## Validation

- `cargo nextest run -p merman-render flowchart_html_unicode_block_fallback_widths_match_upstream default_font_flowchart_html_width_overrides_match_upstream default_font_html_hyphenated_compound_wraps_like_browser`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter stress_flowchart_unicode_punct_in_ids_labels_035 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter stress_flowchart_long_labels_punctuation_unicode_006 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails as expected with 67 Flowchart strict root-only mismatches, down from 69.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.

## Follow-Up

Continue M15C-070 by sampling the remaining leading Flowchart strict-root residuals. Current top
rows include icon-only root metrics, styled-text/root size, Flowchart parameters, shape-mix, demo
flowchart 010/049, and markdown/html=false new/old shape rows.

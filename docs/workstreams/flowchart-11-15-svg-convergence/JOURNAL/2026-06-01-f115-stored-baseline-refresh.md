# F115-080 Stored Flowchart Baseline Refresh

Date: 2026-06-01

## Summary

Stored Flowchart upstream SVG baselines have been refreshed to Mermaid 11.15 after the fresh
Flowchart gate reached zero supported DOM mismatches.

The refresh updated 1069 SVG files and removed 4 stale parser-only KaTeX SVG baselines that Mermaid
11.15 no longer regenerates. The stored baseline directory now contains 1070 Flowchart SVG files.

## Evidence

- `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out fixtures/upstream-svgs`:
  command wrapper timed out after 15 minutes, but the original `xtask` process continued writing
  files and then exited.
- `cargo nextest run -p xtask upstream_svg_baseline_skip_reason svg_xml_compare_skip_reason`:
  passed, 4 tests.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed; `target/compare/flowchart_report.md` reports `All fixtures matched`.
- `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --dom-mode parity --dom-decimals 3`:
  passed; `target/compare/xml/xml_report.md` reports `Mismatches (0)`.
- `cargo nextest run -p merman-render flowchart`: passed, 87 tests.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  failed only for ER and Class; Flowchart is no longer in the full-gate failure set.

## Follow-Up

Close the Flowchart child lane, then continue the umbrella Mermaid 11.15 campaign on the remaining
ER/Class failures.

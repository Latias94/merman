# M15C-070 Flowchart SVG Markdown Root Geometry

## Summary

The largest Flowchart new-shape `parity-root` drift was a real layout/render metrics split, not a
root override candidate. For `htmlLabels=false` markdown node labels, the Flowchart SVG renderer
already wraps markdown rows like Mermaid 11.15, but layout still measured the unwrapped markdown
line before computing shape dimensions. That over-sized triangle/manual-input/tilted-cylinder/
flipped-triangle nodes and propagated into Dagre spacing and root viewBox width.

The fix routes SVG-like markdown labels through the wrapped markdown metrics path before shape
sizing. This keeps Dagre dimensions consistent with the SVG markdown rows emitted at render time.

## Evidence

- Reproduced on
  `upstream_cypress_newshapes_spec_newshapessets_newshapesset1_tb_md_html_false_006`.
- Before the fix, the representative fixture had local max-width `2102.630px` versus upstream
  `1173.540px` (`+929.090px`).
- After the fix, the same no-root-override compare reports local max-width `1172.200px` versus
  upstream `1173.540px` (`-1.340px`).
- `debug-flowchart-layout --text-measurer vendored` now reports the large shape widths near the
  upstream geometry: `n00=242.519`, `n11=196.156`, `n22=218.761`, `n33=245.191`.
- Full Flowchart `parity-root` still fails with 205 strict root-only mismatches. This slice reduced
  magnitude rather than count; the largest remaining Flowchart residuals are now handdrawn,
  long-name, math, and shape-alias buckets.

## Validation

- `cargo nextest run -p merman-render flowchart`: passed, 88 tests.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  failed with the expected remaining 205 Flowchart root-only mismatches.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

## Next

Continue M15C-070 by sampling the new top Flowchart root buckets before adding new root pins:
handdrawn long-name fixtures, `upstream_docs_math_flowcharts_001`, and shape-alias root deltas.

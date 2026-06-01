# 2026-06-01 - M15C-070 Flowchart KaTeX CSS Root Slice

## Context

After the long-name C1 slice, the largest Flowchart strict-root residual was
`upstream_docs_math_flowcharts_001`: upstream root `621.953x178.500`, local root
`556.969x171.375`, delta `-64.984px`. Label diagnostics showed every large delta was a math
`foreignObject` measurement, especially the matrix label (`267.484x25.156` upstream versus
`220.344x20.656` local).

## Diagnosis

The Node/Puppeteer KaTeX probe rendered MathML inside the Mermaid CLI browser shell, but it did not
load `katex/dist/katex.css` before measuring. Mermaid 11.15's browser measurement inherits KaTeX
CSS on `.katex`, so the local probe was effectively measuring bare MathML and underestimating
Flowchart math labels.

## Change

`katex_flowchart_probe.cjs` now loads the local KaTeX stylesheet before Flowchart and Sequence HTML
math measurement. A Rust regression assertion was added for the docs matrix label so the probe
stays near the Mermaid 11.15 browser measurement range instead of the old `220px`-wide fallback.

## Evidence

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_docs_math_flowcharts_001 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed; the fixture now reports `+0.000px` root delta with root overrides disabled.
- `cargo nextest run -p merman-render node_katex_math_renderer_measures_sanitized_flowchart_browser_shell`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still failed with 202 Flowchart root-only DOM mismatches. The largest remaining buckets are
  shape-alias, hexagon, markdown-subgraph, and shape-family geometry/root residuals.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.
- `cargo fmt --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.
- `git diff --check`: passed.

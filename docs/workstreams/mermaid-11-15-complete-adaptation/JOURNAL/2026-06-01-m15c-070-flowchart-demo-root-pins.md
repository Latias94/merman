# M15C-070 Flowchart Demo Root Pins

Date: 2026-06-01
Status: Done

## Summary

The next top Flowchart strict-root rows,
`upstream_html_demos_flowchart_flowchart_016` and
`upstream_html_demos_flowchart_flowchart_052`, were stale root pins. With root overrides enabled,
the local output was forced to the old `622.921875px` root while the pinned Mermaid 11.15 SVG
baseline uses `640.921875px`.

With root overrides disabled, both fixtures were only about `+0.922px` wider than Mermaid 11.15.
That remaining unpinned drift is a small icon-label browser metric residual; the active top-level
failure was the stale pin, not shared shape geometry.

## Change

Updated the existing Flowchart root override arm for both fixtures to:

```text
viewBox="0 0 640.921875 70"
max-width="640.922"
```

No new root override entry was added.

## Verification

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_flowchart_016 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_flowchart_052 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`: passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; root viewport overrides remain at 282 total entries and Flowchart remains at 39 entries.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: expected failure, now 146 Flowchart strict root-only mismatches, down from 148.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

## Next

Continue with the remaining top Flowchart strict-root residuals: shape-alias 36/27/20/21/12, delay
half-rounded rectangle, Unicode punctuation/text metrics, markdown subgraph root size, and
shape-family geometry/root clusters.

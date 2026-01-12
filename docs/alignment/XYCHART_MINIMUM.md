# XYChart Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for XYChart parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `xychart` and `xychart-beta` (case-insensitive).
  - Optional orientation immediately after header:
    - `horizontal` or `vertical`
- Statement separators:
  - newline or `;`
- Common metadata:
  - `title ...`
  - `accTitle: ...`
  - `accDescr: ...` and `accDescr{...}` (supports multiline via brace capture)
  - Last assignment wins.
- Axes:
  - `x-axis`:
    - title only: `x-axis xAxisName`
    - band categories: `x-axis [cat1, "cat 2"]`
    - band + title: `x-axis "x axis" [cat1, cat2]`
    - linear range: `x-axis 1 --> 10`
    - linear + title: `x-axis xAxisName 1 --> 10`
  - `y-axis` (linear only):
    - title only: `y-axis yAxisName`
    - range only: `y-axis 0 --> 100`
    - range + title: `y-axis yAxisName 0 --> 100`
- Plots:
  - `line` and `bar`:
    - `line [1, 2, 3]`
    - `line "title" [ +1, -2, .33 ]`
    - `bar ...` with the same rules
  - Plot data lists must be non-empty and contain valid numbers.

## Derived DB behavior (Phase 1)

- Axis titles and band categories are trimmed and sanitized (mirrors Mermaid `xychartDb.ts`).
- If axes are not explicitly set, plot insertion auto-derives:
  - X axis range: `1..data.length`
  - Y axis min/max from plot data (accumulated across plots unless explicitly set)
- Plot numeric values are transformed into category pairs based on X axis type:
  - band: `[(category[i], value[i])]`
  - linear: categories interpolated between `min..max`

## Output shape (Phase 1)

- Headless semantic output:
  - `type`
  - `title`, `accTitle`, `accDescr`
  - `orientation`
  - `xAxis`, `yAxis`
  - `plots`: each plot includes `type`, `values`, and computed `data` pairs
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `xychart` grammar and DB behavior
compatibility at the pinned baseline tag.


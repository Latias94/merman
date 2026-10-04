# XYChart Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for XYChart parsing in `merman`.

Baseline: Mermaid `@11.12.3`.

Upstream references:

- Parser grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/xychart/parser/xychart.jison`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/xychart/parser/xychart.jison.spec.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/xychart/xychartDb.ts`

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

## Mermaid 12.1 chart title layout

Source: Mermaid `12.1.0`, commit `21f72f07ea22c0af48a3149c550654e80d8e40cb`,
`packages/mermaid/src/diagrams/xychart/chartBuilder/components/chartTitle.ts`,
`orchestrator.ts`, and `textDimensionCalculator.ts`.

- A chart title must fit, including both `titlePadding` margins, in the height left after the
  plot's reserved-height allocation. If it does not fit, no title drawable or SVG group is emitted.
- In both orientations the title centers on the final plot area, excluding axis and legend width.
  It has a separate row above the plot and legend, so a wide title may span their columns.
- The center is constrained by the measured title width and the full chart width. A title wider
  than the chart is centered on the chart; the renderer does not invent truncation or wrapping.
- Merman consumes text metrics directly in SVG user units. Mermaid's browser implementation
  divides screen-space measurements by the SVG CTM's separate horizontal and vertical scales;
  headless layout has no screen transform to apply or reverse. Host measurement providers must
  return layout-space metrics. Font-specific metric differences remain an artifact boundary.

These are existing XYChart layout behaviors, with no syntax, semantic model, feature, or dependency
addition.

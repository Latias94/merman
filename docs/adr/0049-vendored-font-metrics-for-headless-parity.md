# ADR 0049: Vendored Font Metrics for Headless SVG DOM Parity

Date: 2026-01-19

## Context

Mermaid measures SVG text using the browser rendering engine:

- `calculateTextWidth` / `calculateTextHeight` call `calculateTextDimensions`
- `calculateTextDimensions` appends an SVG element to `document.body` and uses `getBBox()`
- This depends on the user agent, available fonts, font fallback, and sub-pixel rounding

In a headless Rust library, we cannot rely on a browser DOM, and we intentionally avoid introducing
platform-dependent system font discovery as a hard requirement.

However, Mermaid emits some font-metric-derived values directly into the SVG DOM, notably:

- the C4 type line uses `<text ... lengthAdjust="spacing" textLength="..."><<type>></text>`

If we do not match these values, we cannot reach stable SVG DOM parity with the pinned upstream
baselines.

## Decision

For Mermaid `@11.12.2`, `merman-render` vendors a small set of DOM-derived font metrics that are:

- required for SVG DOM parity,
- stable across the pinned upstream baselines,
- and tied to a specific Mermaid version and diagram subsystem.

Currently, this is limited to **C4 type-line `textLength`** values for built-in shape types.

The values are generated from upstream SVG baselines via:

- `cargo run -p xtask -- gen-c4-textlength`

The generated output is checked by:

- `cargo run -p xtask -- verify-generated`

## Alternatives Considered

1. **Implement true text measurement in Rust (font parsing + shaping + metrics)**  
   Pros: future-proof, not limited to one diagram.  
   Cons: requires bundling fonts (or depending on system fonts), shaping behavior and fallback can
   still differ from browser engines, and adds significant complexity/maintenance.

2. **Use a headless browser to compute metrics during rendering**  
   Pros: closest to Mermaid.  
   Cons: defeats “headless library” constraints, introduces heavy runtime dependencies.

3. **Ignore metric-derived DOM attributes in parity mode**  
   Pros: simplifies comparisons.  
   Cons: diverges from “1:1 parity” goals; regressions become harder to detect.

## Consequences

- We gain deterministic SVG DOM parity for C4 fixtures under the pinned Mermaid version.
- We accept that metric vendoring is **version-scoped** and must be regenerated when baselines
  update.
- We keep the scope small and explicitly documented to avoid a “vendored metrics sprawl”.

## Follow-ups

- Extend the generator to cover additional metric-derived attributes only when they block parity.
- Re-evaluate full Rust-side text measurement if we need robust parity across arbitrary fonts or
  user-specified font overrides.


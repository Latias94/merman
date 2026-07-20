# ADR-0059: SVG, Raster, and Vector PDF Export Strategy

Date: 2026-02-09

Amended: 2026-07-20

Status: Accepted

## Context

Merman targets source-backed behavior from Mermaid `@11.16.0`. The primary parity contract is SVG
DOM structure against pinned upstream baselines. Applications also need PNG, JPEG, and PDF export
without bundling a browser.

Upstream Mermaid relies heavily on HTML inside SVG `<foreignObject>` labels. Pure-Rust SVG
consumers do not implement the complete browser HTML/CSS layout model, so passing parity SVG
directly to an image or PDF backend can lose labels. Output dimensions also have fundamentally
different costs: an SVG or vector PDF page can describe large geometry without allocating a
full-page pixmap, while PNG and JPEG must allocate every output pixel.

Treating all exports as one raster policy therefore gives the wrong behavior. It either rejects
valid vector output or permits unsafe bitmap allocations.

## Decision

Merman keeps one renderer-owned export preparation path and distinct output allocation policies.

1. Default SVG output preserves the Mermaid-parity contract. It remains vector markup with no
   global width or height cap; normal source, model, label, and SVG-byte resource budgets still
   apply.
2. PNG/JPEG and PDF export start from the terminal `SvgPipeline::resvg_safe()` policy. It converts
   supported `<foreignObject>` labels to SVG text fallbacks, removes the original HTML, strips
   active or unsupported content, parses the result, and returns a sealed `ResvgCompatibleSvg`.
3. PNG and JPEG use `RasterOptions`. Fit, scale, final dimensions, and embedded raster images are
   planned before allocation. Safe defaults limit each side to 4096 pixels and the final image to
   16,777,216 pixels.
4. PDF uses independent `PdfOptions` and a Rust vector PDF backend. Page geometry does not share the
   PNG/JPEG pixel budget. SVG filters that require localized bitmaps and embedded raster images
   have separate aggregate limits.
5. Browser-style PDF fitting is a page-unit contract, not a raster-size heuristic.
   `PdfPagePolicy::FitCssWidth` constrains responsive SVG width in CSS pixels and converts 96 CSS
   pixels to 72 PDF points. Top-level `merman-cli --pdfFit` uses this policy; the default top-level
   page remains a 612-by-792-point Letter approximation.
6. Each unbounded option disables only its named allocation boundary. No export option disables
   parser, layout, label, or SVG-byte resource profiles.
7. Resvg-compatible export has a separate backend capability boundary for resolved SVG group
   depth. Native prepare/encode work runs on an 8 MiB worker stack and accepts 256 levels; the
   WebAssembly build accepts 64 levels because it cannot create that worker stack. The boundary
   applies after `usvg` resolves references as well as to the final XML tree, and cannot be removed
   by an unbounded output option.

PNG/JPEG and PDF remain best-effort integration outputs rather than pixel-parity gates. Their SVG
geometry and source semantics should converge with Mermaid, but browser font layout, rich HTML,
filters, Chromium screenshots, and print behavior may retain documented residuals.

## Alternatives Considered

### Bundle headless Chromium

This would most closely reproduce `<foreignObject>`, browser fonts, screenshots, and print-to-PDF.
It was rejected because of its large dependency footprint, startup cost, packaging complexity, and
conflict with the browserless Rust architecture.

### Apply one pixel limit to PNG, JPEG, and PDF

This would offer a superficially simpler option surface. It was rejected because a vector PDF page
does not allocate a full-page pixmap. A shared limit would reject valid large vector documents while
failing to express the real PDF costs: localized filter bitmaps and embedded raster images.

### Implement a complete HTML/CSS engine for `<foreignObject>`

This would keep the runtime pure Rust and could improve browser parity. It was rejected as a
separate browser-layout project with disproportionate maintenance cost. Source-backed family text
rendering and narrow export fallbacks remain preferable.

### Modify default parity SVG with fallback wrappers

Adding `<switch>` or duplicate fallback text to every SVG would help some consumers. It was rejected
because it changes the upstream SVG DOM contract. Export cleanup remains explicit.

## Consequences

- Large finite SVG and vector PDF dimensions are supported without a global side-length heuristic.
- PNG/JPEG allocation is predictable and inspectable through `RasterPlan` before a pixmap exists.
- PDF filter sampling and embedded image decoding are independently bounded and inspectable.
- Hosts must choose the output policy that matches their display or export target instead of using
  one unbounded switch for every format.
- Rich browser-only labels and print behavior can still differ, and those differences must remain
  documented rather than hidden by broad comparator normalization.

## References

- `docs/rendering/RASTER_OUTPUT.md`
- `docs/security/THREAT_MODEL.md`
- `docs/alignment/CLI_COMPATIBILITY.md`

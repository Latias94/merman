# ADR-0059: Raster Output Strategy (PNG/JPG/PDF) for `merman-cli`

Date: 2026-02-09

## Context

This repository targets a 1:1 re-implementation of Mermaid pinned to Mermaid `@11.12.2`.

The primary parity contract is **SVG DOM parity** against upstream baselines stored under
`fixtures/upstream-svgs/**`.

For developer ergonomics and downstream integrations, `merman-cli` also exposes raster formats:

- PNG
- JPG
- PDF

Upstream Mermaid renders in a browser and heavily uses HTML labels via SVG `<foreignObject>`.
Pure-Rust SVG raster stacks (`usvg`/`resvg`/`svg2pdf`) do not fully render `<foreignObject>`,
resulting in “missing text” (or effectively blank) raster outputs for many diagram types.

## Decision

`merman-cli` will treat raster output as **best effort** and will **not** require pixel-perfect
parity with upstream browser rendering.

For raster formats only, `merman-cli` will apply an SVG preprocessing step that:

- replaces common `<foreignObject>` label patterns with SVG `<text>/<tspan>` equivalents
  (approximate centering + line breaks), and
- leaves SVG output unchanged to preserve the upstream SVG baseline contract.

## Alternatives considered

1. Bundle headless Chromium (Puppeteer-like) for raster output
   - Pros: closest to upstream rendering semantics (`foreignObject`, CSS, fonts).
   - Cons: large dependency footprint, slower startup, harder cross-platform distribution,
     weak alignment with “pure Rust headless library” goals.

2. Implement a full HTML/CSS layout/rendering engine for `<foreignObject>`
   - Pros: pure Rust, potentially fully deterministic.
   - Cons: significant scope and long-term maintenance burden; effectively re-implementing parts
     of a browser.

3. Emit `switch(foreignObject, text)` in the default SVG output
   - Pros: would help rasterizers that support `<switch>`.
   - Cons: breaks upstream SVG DOM parity (upstream does not emit these wrappers).

## Consequences

- Raster output becomes useful immediately for previews and CI artifacts.
- Raster output is explicitly not a parity gate; SVG remains the spec.
- Some rich label rendering will degrade in raster outputs until specialized conversions are added.

## References

- `docs/rendering/RASTER_OUTPUT.md`


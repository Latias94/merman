# ADR-0057: Headless SVG Text `getBBox()` Approximation

## Status

Accepted

## Dates

- Proposed: 2026-02-03
- Updated: 2026-07-15

## Baseline

Mermaid `@11.16.0`

## Context

Mermaid derives many diagram root viewports from browser DOM measurement.
`setupGraphViewbox(svg, padding, useMaxWidth)` reads `svg.getBBox()`, then applies padding and
responsive sizing. Text contributes browser-dependent values affected by:

- font family, size, weight, fallback, shaping, and hinting;
- Mermaid whitespace tokenization and `<tspan>` generation;
- anchor, baseline, transforms, and inherited presentation attributes; and
- browser float lattices and serialization.

Merman is headless. It cannot reproduce a complete browser CSS and SVG text engine, but it must
produce deterministic layout and root viewport evidence without allowing each family to choose an
untracked measurement implementation.

## Decision

### Measurement policy belongs to the render operation

`RenderEnvironment` selects a named `TextMeasurementPolicy` before rendering starts. The policy
routes the `Layout`, `Wrap`, `SvgBBox`, `ComputedLength`, and `Visibility` phases independently.
`begin_session()` freezes those routes once for the whole operation and records their provenance.

The parity environment uses the pinned vendored measurement profile. Hosts may install an explicit
host profile or phase-specific route, but a family renderer must not construct or silently select a
production measurer. Architecture, Treemap, and other measurement-sensitive families consume the
operation's routed measurer like every other family.

### `getBBox()` approximation remains a layered model

1. **Geometry layer**
   - Parse emitted SVG and union bounds for the geometry Merman emits, including rectangles,
     circles, ellipses, lines, paths, polylines, polygons, and `foreignObject`.
   - Preserve supported transform stacking and match whole attribute names.
   - Keep this layer deterministic and independent from CSS text layout.

2. **Text layer**
   - Families whose emitted text lacks concrete surrounding geometry must contribute a text-bound
     estimate through the `SvgBBox` measurement route.
   - The vendored profile uses Mermaid-like SVG text extents and whitespace tokenization.
   - Families may own higher-level bounds, such as Architecture service-label bounds, rather than
     teaching the generic geometry pass every nested `<tspan>` and inherited style behavior.

3. **Root viewport layer**
   - Families pass their computed content bounds and source-backed root algorithm to the shared Root
     Viewport module.
   - Root Viewport owns finite normalization, padding, sizing, max-width formatting, generated
     override resolution, and root SVG emission.
   - Browser-only residuals may use version-pinned, auditable root evidence under ADR-0062. They do
     not justify a hidden family measurer or a model-level geometry distortion.

## Alternatives Considered

### Full XML, CSS, font fallback, and SVG text layout engine

Rejected. It is a browser implementation project with a large dependency and divergence surface,
not a bounded headless renderer capability.

### Ignore text and rely on container geometry

Rejected. Mermaid emits visible text without usable container dimensions in families such as
Architecture, so the resulting root viewport can clip content.

### Parse every `<text>` and `<tspan>` generically after emission

Rejected as the default. Correct handling would require the style cascade, inheritance, nested
positioning, font fallback, and browser quantization. Family-owned text bounds are narrower and
more source-backed.

### Let each family instantiate the measurer it needs

Rejected. It makes parity depend on hidden adapter selection and prevents operation-level
provenance, host injection, and reproducible verification.

## Consequences

- Text measurement is explicit, phase-aware, and observable for one complete operation.
- Hosts can replace documented phases without accidentally leaving another family on a hidden
  vendored adapter.
- Generic geometry bounds remain stable while measurement-sensitive families contribute precise
  source-backed bounds.
- Root viewport behavior has one policy owner even though family content-bounds algorithms differ.
- Some residual values remain browser-dependent. They must be documented or governed by ADR-0062,
  not hidden by broad comparator normalization.

## Related Decisions

- ADR-0049: Vendored Font Metrics for Headless Parity
- ADR-0050: Release Quality Gates
- ADR-0062: Fixture-Derived Overrides
- ADR-0073: Family-Owned Diagram Architecture

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
routes the `Layout`, `Wrap`, `SvgBBox`, and `ComputedLength` phases independently.
`begin_session()` freezes those routes once for the whole operation and records their provenance.

The parity environment uses the pinned vendored measurement profile. Hosts may install an explicit
host profile or phase-specific route, but a family renderer must not construct or silently select a
production measurer. Architecture, Treemap, and other measurement-sensitive families consume the
operation's routed measurer like every other family.

A successful host result is authoritative for the requested phase and DOM operation. It bypasses
vendored calibration entirely; vendored facts are only the deterministic fallback for an absent,
failed, or invalid host result according to the configured route. Distinct browser operations such
as direct `<text>.getBBox()`, `<text><tspan>.getBBox()`, and
`<text>.getComputedTextLength()` remain distinct measurement operations instead of being collapsed
into one approximate width.

Operation 18, `raw-bbox-height`, owns the non-negative height returned by
`<text>.getBBox().height` for a raw text node with its effective font family, size, weight, and
style. Callers such as TreeView use this operation because Mermaid lays out each row from that
exact browser result. A host must not answer it with tspan bounds, generic line height, or a
font-size multiplier.

The vendored profile mirrors that distinction. Its internal `MRMFNT05` blob stores four named SVG
vertical DOM shapes: direct raw `<text>`, `<text>` with one child `<tspan>`,
`createFormattedText(...)`, and the Architecture middle-baseline form of
`createFormattedText(...)`. Raw operation 18 and the tspan bbox operation perform separate shape
lookups. Neither trait defaults nor runtime helpers silently substitute one profile for the other.

The generator probes each shape over the canonical font-stack, variant, integer-size, and glyph
domain and proves glyph-union composition with pair strings. It emits an explicit shape alias only
when every canonical fact is bit-identical. Otherwise the shape has an independent profile. The
`mermaid-calculate-text-dimensions` profile remains operation-owned and reads its body-attached
single-tspan facts rather than borrowing the direct-text profile.

`MRMFNT05` is an internal generated-data schema, not a public binding ABI. The public alpha ABI
remains version 2.

### Baseline-bearing DOM probes remain distinct

Mermaid Architecture creates a formatted SVG label and then applies
`alignment-baseline="middle"`, `dominant-baseline="middle"`, and `text-anchor="middle"` to its
outer label group. The inherited middle baseline changes the descendant `<text>.getBBox().y`
according to the resolved font's baseline and x-height. It is therefore a different browser
primitive from an ordinary `createFormattedText(...)` bbox y even when the text and font style are
otherwise identical.

The host protocol exposes the ordinary `createText(...)` SVG path's bbox y (implemented internally
by `createFormattedText(...)`) as operation 14,
`create-text-bbox-y-offset`, and the Architecture middle-baseline variant as operation 17,
`create-text-middle-bbox-y-offset`. Both return finite signed lengths. Browser hosts must measure
the exact DOM shapes using isolated probes, or clear inherited baseline and anchor state between
requests; operation 14 must not be reused as the answer for operation 17.

The vendored profile stores ordinary and middle-baseline bbox y/height facts independently using
the actual nested outer/inner tspan DOM. It does not derive either answer from raw text bounds, a
probe glyph such as `M`, or a middle-baseline shift formula. Glyph-union and string-composition
proofs gate exact profile use in the same way as the raw and single-tspan shapes.

Inputs outside the canonical integer-size and glyph domain use a separately named approximate
high-resolution fallback. That fallback is deterministic but is not described as an exact browser
fact. A successful host result remains authoritative and bypasses it.

### `getBBox()` approximation remains a layered model

1. **Geometry layer**
   - Parse emitted SVG and union bounds for the geometry Merman emits, including rectangles,
     circles, ellipses, lines, paths, polylines, polygons, and `foreignObject`.
   - Preserve supported transform stacking and match whole attribute names.
   - Keep this layer deterministic and independent from CSS text layout.

2. **Text layer**
   - Families whose emitted text lacks concrete surrounding geometry must contribute a text-bound
     estimate through the `SvgBBox` measurement route.
   - The vendored profile uses Mermaid-like SVG text extents and whitespace tokenization. Its
     generated data is limited to general font and DOM-shape facts such as glyph advances, kerning,
     trigrams, and endpoint overhangs. Complete label strings and fixture ids are not valid keys.
   - Synthetic browser probes generate fallback facts; the fixture corpus validates them
     independently and does not train full-string answers.
   - Families may own higher-level bounds, such as Architecture service-label bounds, rather than
     teaching the generic geometry pass every nested `<tspan>` and inherited style behavior.

3. **Root viewport layer**
   - Families pass their computed content bounds and source-backed root algorithm to the shared Root
     Viewport module.
   - Root Viewport owns finite normalization, padding, sizing, max-width formatting, and root SVG
     emission from computed family or emitted-content bounds.
   - Browser-only residuals remain verification evidence under ADR-0062. They do not justify a
     fixture pin, hidden family measurer, or model-level geometry distortion.

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
  vendored adapter, and successful host results bypass fallback facts.
- Generic geometry bounds remain stable while measurement-sensitive families contribute precise
  source-backed bounds.
- Root viewport behavior has one policy owner even though family content-bounds algorithms differ.
- Some residual values remain browser-dependent. They must be documented under ADR-0062, not
  hidden by production lookup tables or broad comparator normalization.

## Related Decisions

- ADR-0049: Vendored Font Metrics for Headless Parity
- ADR-0050: Release Quality Gates
- ADR-0062: No Production Fixture Overrides
- ADR-0073: Family-Owned Diagram Architecture

# XY Chart text glow: preparation boundary

Date: 2026-09-18. Investigated at `699933336` during U8.
Status: design findings, not an implemented or qualified consumer.

## Follow-up correction: ordinary SVG and native observation

A follow-up source review at `0bf29d0f9` found that requiring prepared native labels before
ordinary SVG text glow would contradict R4 and AE2. The preparation proposal below is therefore
conditional on an explicitly resource-backed text path, not a prerequisite for rendering glow.
`RenderEnvironment::try_native()` changes runtime policy; it is not a native font measurement
profile. Ordinary XY layout continues to use its selected `TextMeasurer` unchanged.

The existing SVG host can resolve an object-bounding-box text filter. A region derived from
layout dimensions allocates an effect surface; it does not attest actual glyph ink. Ordinary
SVG can retain host-dependent text provenance without loading font resources. Region sizing and
root paint expansion still need the actual browser scenes; a fixed oversized percentage is not
an acceptable substitute.

The existing native export stage already owns the final `usvg::Tree`, font database and filter
receipt. Its object-bounding-box mapping check uses the correct SVG metrics rectangle, but does
not establish that the actual glyph glow fits. Add text containment there, reusing native glyph
outlines and the ordered four-sigma shadow envelope. `Text::stroke_bounding_box()` is populated
from flattened glyph outlines; `Text::bounding_box()` instead follows SVG text-metrics semantics.
Check the local filter rectangle and the actual exporter-selected output viewport. PNG with a
viewBox and PDF use `tree.size()`; raster output without a viewBox uses the original-coordinate
content crop selected by `raster_geometry_for_svg()`. usvg has already applied any root viewBox
transform. Unknown ancestor clipping/masking cannot be certified
from an axis-aligned clip bounding box. Keep traversal linear and shape-only behavior unchanged.
This is an extension of existing target observation, not a new receipt system. System fonts
remain host-dependent even when containment passes.

The exporter containment correction and negative tests are recorded in the
[native containment verification](2026-09-18-native-text-shadow-containment.md). They do not by
themselves implement a text writer or close U8. A later exact resource-backed XY layout can use
the preparation route below, but it must apply consistently with or without an effect.

## Conditional resource-backed preparation boundary

The current XY layout uses `execution.text_measurer()` through `max_text_dimension` and
`single_text_height`. Its layout dimensions are not proof of font ink. The separate retained
catalog preparation session must not silently replace an explicit host measurer; the existing
`environment.rs` regression intentionally preserves that distinction.

Use the existing prepared text session, native backend, work meter and label ledger. A private
XY artifact should retain final role text, font, prepared advance/ink and actual writer placement.
Do not prepare the same text under a second unrelated profile merely to obtain an effect box.
Do not introduce a public measurement trait or a separate proof framework.

A required design check remains before implementation: adding only an effect must not change
layout measurement. If the existing explicit native profile cannot supply the same prepared
result to both layout and writer, establish that profile's XY text path for both the control and
effected scene. Do not conditionally switch metrics only when glow is requested. Merely retaining
a catalog is not authorization to replace a user-supplied measurer. Keep the ordinary no-effect
path free of effect preparation and per-label receipt payloads.

## Coordinates and baseline metrics

`PreparedTextLine` already carries advance, horizontal ink origin and vertical ink extents
relative to an alphabetic baseline. XY emits start/middle/end text anchors and middle or
text-before-edge dominant baselines, sometimes with a rotated axis title.

The inspected usvg 0.47 implementation uses x-height/2 for middle and ascent for text-before-edge.
Ink height/2 and layout height are not interchangeable with those metrics. Obtain necessary
baseline data from the actual selected native font face through private backend data; XY must
not duplicate font selection or guess from a nominal line box. Font fallback and mixed-face
text must be handled consistently with the declared target, or retain explicit residuals.

Construct filter regions in the text's local coordinates. Apply the writer's real rotation and
translation only when unioning the four filter-region corners into the root paint bounds.
Use `SvgShadowEffect::from_graph`, `materialize_user_space`, the shared SVG shadow writer and
`SvgShadowEvidenceRecorder`. Keep the existing `-theme-effect-` ID convention required by native
export discovery. Font/text/transform consumption still needs the prepared label ledger; a
matching filter definition alone cannot prove text geometry.

The narrow code seams are XY typography/artifact/layout/writer, family preparation and ledger
aggregation, native prepared baseline data, and the existing family mechanism/support matrix.
Do not alter all environments whenever a theme exists, add font dependencies or bundle a font.

## Reference blur semantics

The pinned modern_mermaid reference uses CSS `text-shadow` on Title (15px, alpha .8), AxisTitle
(10px, alpha .6) and Legend (8px, alpha .5), all with opaque cyan text. CSS Text Decoration 3
section 4 gives text-shadow the box-shadow value interpretation. CSS Backgrounds 3 section
6.1.2 defines the Gaussian standard deviation as half the blur radius. Filter Effects 1 defines
the third drop-shadow length directly as standard deviation.

Therefore the current effect graph's SVG sigma values must be **7.5, 5 and 4**, respectively.
Do not copy the three CSS text-shadow radii into SVG sigma unchanged. This correspondence is
for the reference's opaque glyph scene; a SourceGraphic drop shadow is not a general CSS
text-shadow interpreter, including for partially transparent text.

Primary specifications checked:

- [CSS Text Decoration 3, text shadows](https://www.w3.org/TR/css-text-decor-3/#text-shadow-property)
- [CSS Backgrounds 3, shadow blur](https://www.w3.org/TR/css-backgrounds-3/#shadow-blur)
- [Filter Effects 1, drop-shadow](https://www.w3.org/TR/filter-effects-1/#funcdef-filter-drop-shadow)

## Acceptance constraints

For the XY writer, initially implement the requested Title, AxisTitle and Legend roles. Do not widen that
claim to tick labels, point labels, arbitrary host backends or other families. Unknown ink or
baseline ownership must remain Unsupported/Unverified rather than receiving a fabricated box.
However, a Latin-only or single-face probe does **not** complete U8: the declared public scenes,
including their actual text and fallback requirements, must work before the unit closes.

Required checks include:

- unchanged layout when only glow is added; no-effect controls use the same measurement profile;
- overhangs, descenders, start/end anchors, rotated titles and multilingual/fallback scenes;
- hidden, empty or nonfitting roles, Clear, later winners and property-local source overrides;
- font/text/size/weight/transform mismatches, wrong filter ID, omitted terminal consumption,
  missing prepared label markers and postprocessing invalidation;
- catalog/profile mismatch, fake native identity, unavailable font or unsupported host response;
- actual SVG/PNG/PDF output, matching font/filter receipts and effect resource ceilings.

The next implementation must inspect these seams directly and verify the measurement-profile
invariant before choosing a data shape. This note records constraints, not approval to narrow
the public scene contract or promote catalog qualification.

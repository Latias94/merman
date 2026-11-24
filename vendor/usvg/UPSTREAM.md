# usvg 0.47.0 backport

Source: the crates.io `usvg` 0.47.0 archive, SHA-256
`d46cf96c5f498d36b7a9693bc6a7075c0bb9303189d61b2249b0dc3d309c07de`,
built from [resvg `3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707`](https://github.com/linebender/resvg/tree/3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707/crates/usvg).
The Cargo manifest disables publication. The runtime source and both upstream license files
are copied from the archive.

The change in `src/text/layout.rs` shapes missing text in its original direction and
surrounding context. Consecutive missing clusters with equivalent physical faces, effective
coordinates, sizes and scripts are shaped together, preserving ligatures and joining. Glyphs
already supplied by the primary face keep their original positions within their cluster.
Adjacent spans with matching shaping properties and equivalent language tags share a run.
`Font::selection_language` supplies the effective language to primary family matching.
`Options::set_text_fallback` lets the font host consider the authored family stack,
inherited `xml:lang`, and complete cluster text. `Options::set_font_instances` also
carries native design coordinates and relative size in primary and fallback selections.
The fallback callback receives the original selections in order, beginning with the primary
instance. Layout deduplicates attempts by physical face, effective coordinates and size.
Nested SVGs inherit the selectors and default language. The scalar and cluster selectors
remain available for existing callers. Upstream
[resvg #1087](https://github.com/linebender/resvg/pull/1087) discusses cluster replacement.

Effective coordinates are resolved against the physical face. Explicit variation settings
override authored font properties, followed by native coordinates and face defaults.
An omitted weight uses CSS normal (400) when the native selection supplies no weight.
Automatic optical sizing uses the scaled font size, and `none` retains the native optical
coordinate. Explicit `opsz` takes precedence in either mode. The same coordinates are used
for shaping, active face metrics and outlines. Glyph bounds use the glyph's effective instance.
Baselines and decorations use the effective primary instance for the styled span. SVG layout
passes the resulting glyph coordinates and size to PDF-facing spans, with default coordinates
omitted. Outline caching still applies to the default instance.

The Merman workspace patches one usvg source for native measurement, raster, and vector PDF.
Remove the corresponding patches when a common supported release provides these fixes.

The native metric probe reuses a single text shape to obtain start- and end-anchor bounds.
It retains the normal positioning and outline-conversion checks for both bounds. Merman's
registry builds use the two-element probe with the published usvg API.

Text conversion records visible spans without a usable primary face and missing glyphs in
text nodes that are dropped during conversion. The exporter combines these omissions with
the final positioned glyphs to check strict coverage. Individual resolver misses during
speculative shaping can be recovered by later spans or fallback faces.

Clip paths, masks, patterns and markers that become empty after text conversion carry font
omissions to the element that used them. The exporter follows the retained clip and mask chains
from their visible consumers and recognizes zero-opacity groups and unpainted outline text.
Filter images that lose their text retain the omissions on their transparent image root.

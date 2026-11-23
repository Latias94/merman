# usvg 0.47.0 backport

Source: the crates.io `usvg` 0.47.0 archive, SHA-256
`d46cf96c5f498d36b7a9693bc6a7075c0bb9303189d61b2249b0dc3d309c07de`,
built from [resvg `3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707`](https://github.com/linebender/resvg/tree/3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707/crates/usvg).
The Cargo manifest disables publication. The runtime source and both upstream license files
are copied from the archive.

The change in `src/text/layout.rs` shapes a missing cluster in its original direction and
surrounding context. A successful fallback replaces that cluster and keeps existing glyphs.
Adjacent spans with matching shaping properties share a run. `Options::set_text_fallback`
lets the font host consider the authored family stack, inherited `xml:lang`, and the complete
cluster text. The returned relative size affects glyph advances, outlines and PDF spans.
The scalar resolver remains available for existing callers. Upstream
[resvg #1087](https://github.com/linebender/resvg/pull/1087) discusses cluster replacement.

The CSS weight mapping follows [usvg 0.50.0](https://docs.rs/crate/usvg/0.50.0/source/src/parser/text.rs).
It supplies `wght` for CSS normal (400) as well as other weights. An explicit `wght` takes
precedence. Outline caching still applies to faces at their default coordinates.

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

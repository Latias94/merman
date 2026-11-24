# merman-usvg 0.50.0

The crates.io `usvg` 0.50.0 archive has SHA-256
`bdb896f04730b97805bbc3191fc57e4bf03face0f14577dd42e783c4d8c98802` and was built from
[resvg `5bdbae64eefadc4bf0caa56dcf91702037929086`](https://github.com/linebender/resvg/tree/5bdbae64eefadc4bf0caa56dcf91702037929086/crates/usvg).
The library and both upstream license files come from the archive. The Cargo package uses
Merman's name and keeps `usvg` as its Rust library name. It includes the Merman font instance
regression tests and their generated fixture.

The change in `src/text/layout.rs` shapes missing text in its original direction and
surrounding context. Consecutive missing clusters with equivalent physical faces, effective
coordinates, sizes and scripts are shaped together, preserving ligatures and joining. Glyphs
already supplied by the primary face keep their original positions within their cluster.
Adjacent spans with equivalent resolved fonts, shaping options and language tags share a run.
Fallback uses the original span's request and primary selection for each missing cluster.
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
for HarfRust shaping, Skrifa face metrics and outlines. Baselines and decorations use the
effective primary instance for the styled span. SVG layout passes the resulting glyph
coordinates and size to PDF-facing spans, with default coordinates omitted. The upstream
font cache stores shape plans and outlines per instance.

`merman-export`, `merman-resvg` and `merman-krilla-svg` depend on this package. They share one
SVG tree and font database across native measurement, raster, and vector PDF.

The native metric probe reuses a single text shape to obtain start- and end-anchor bounds.
It retains the normal positioning and outline-conversion checks for both bounds.
Path sampling supports nonmonotonic distances from bidirectional glyph positioning. `textLength`
spacing is applied between clusters so the last cluster's advance also gives the next text chunk
its correct starting position.

Text conversion records visible spans without a usable primary face and missing glyphs in
text nodes that are dropped during conversion. The exporter combines these omissions with
the final positioned glyphs to check strict coverage. Individual resolver misses during
speculative shaping can be recovered by later spans or fallback faces.

Clip paths, masks, patterns and markers that become empty after text conversion carry font
omissions to the element that used them. The exporter follows the retained clip and mask chains
from their visible consumers and recognizes zero-opacity groups and unpainted outline text.
Filter images that lose their text retain the omissions on their transparent image root.

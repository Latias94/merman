# merman-krilla-svg

Merman's SVG-to-PDF adapter uses the krilla-svg 0.8.1 library. Its `SurfaceExt` accepts a
`merman-usvg` 0.50 tree and writes to a `merman-krilla` surface. It preserves selectable
SVG text, the effective OpenType coordinates, and Krilla's color glyph rendering.

See [UPSTREAM.md](UPSTREAM.md) for provenance. The source is available under either the
[Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) license.

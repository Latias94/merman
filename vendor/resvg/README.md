# merman-resvg

Merman's rasterizer uses the resvg 0.50.0 library with `merman-usvg` as its SVG parser and
text shaper. Use `resvg::usvg` to access the exact SVG tree types accepted by `resvg::render`.

Default features enable SVGZ, text, system fonts, font memory mapping, and raster images.

See [UPSTREAM.md](UPSTREAM.md) for provenance. The source is available under either the
[Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) license.

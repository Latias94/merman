# merman-krilla-svg 0.8.1

The library source comes from the crates.io `krilla-svg` 0.8.1 archive with SHA-256
`1237d7c37b16ca9fbc2e72dde13a10d321f9a44aceee35c062bc0a43bfc7ce16`.
The source revision is [Krilla `3ffdf0588cf98050aad6edba51ca70162e1fb5b5`](https://github.com/LaurenzV/krilla/tree/3ffdf0588cf98050aad6edba51ca70162e1fb5b5/crates/krilla-svg).
The license files come from that revision's repository root.

The Cargo manifest uses `merman-usvg`, `merman-resvg` and `merman-krilla` in place of their
upstream counterparts. This keeps SVG, raster filter, and PDF font types consistent.

The PDF adapter uses the fontdb 0.24 database of each SVG tree and the coordinates and
size resolved during layout. Krilla handles PDF font embedding and color glyph drawing.
Filter rasterization follows the optional bounds in usvg 0.50. For vector PDF groups,
the SVG `plus-lighter` blend mode is approximated with PDF `Screen`.

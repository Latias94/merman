# usvg 0.47.0 backport

Source: the crates.io `usvg` 0.47.0 archive, SHA-256
`d46cf96c5f498d36b7a9693bc6a7075c0bb9303189d61b2249b0dc3d309c07de`,
built from [resvg `3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707`](https://github.com/linebender/resvg/tree/3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707/crates/usvg).
The Cargo manifest disables publication. The runtime source and both upstream license files
are copied from the archive.

The change in `src/text/layout.rs` keeps trying fallback faces after an incompatible partial
shape. It can fill a missing cluster when both faces shape the same original byte range.
Upstream [resvg #1087](https://github.com/linebender/resvg/pull/1087) discusses the cluster
boundary problem. The Merman workspace patches one usvg source for native measurement, raster,
and vector PDF. Remove this patch when a common supported release fixes the regression.

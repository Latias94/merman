# merman-resvg 0.50.0

The library source and the Apache-2.0 and MIT license files come from the crates.io `resvg`
0.50.0 archive with SHA-256 `77bb4b284cf8a5c8ae721594af53387fb755d2196fddb32b2acfb4fa9251fa0c`.

The source revision is [resvg `5bdbae64eefadc4bf0caa56dcf91702037929086`](https://github.com/linebender/resvg/tree/5bdbae64eefadc4bf0caa56dcf91702037929086/crates/resvg).

The Cargo manifest uses `merman-usvg` to share the SVG tree with Merman's font resolver and
PDF adapter. The package includes the upstream library and its feature options.

The IIR Gaussian blur in `src/filter/iir_blur.rs` credits Pascal Getreuer's implementation
under the Simplified BSD License. [LICENSE-BSD-2-Clause](LICENSE-BSD-2-Clause) preserves the
copyright notice from `gaussian_20131215/gaussian_conv_am.c` and the verbatim license from
`gaussian_20131215/BSD_simplified.txt` in the [original source archive](https://www.ipol.im/pub/art/2013/87/gaussian_20131215.tgz)
with SHA-256 `9458287ac105d51e60d87a69a6b9107d7540add2fcab362bd5ee18f359219dfd`.

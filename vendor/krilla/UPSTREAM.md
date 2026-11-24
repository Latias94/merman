# merman-krilla 0.8.2

The runtime source and ICC profiles come from the crates.io `krilla` 0.8.2
archive with SHA-256 `27da593198b20eeba65caeb73c2bbeec3e53ab08fa549898312ce81c4fce5e33`.
The archive identifies [upstream `3ffdf0588cf98050aad6edba51ca70162e1fb5b5`](https://github.com/LaurenzV/krilla/tree/3ffdf0588cf98050aad6edba51ca70162e1fb5b5/crates/krilla).
The Cargo package keeps the library name `krilla` and upstream runtime features. Merman's
export integration tests own qualification, including coverage of PDF font subsetting.
The upstream examples and test assets are outside the fork's package.

`src/text/cid.rs` seeds the six-letter subset tag from the font instance and its glyph
indices. The serializer reserves the tag in a document-wide set and resolves collisions.
Each CID subset receives a unique tag even when two instances of one variable face contain
the same glyph indices.

The integration test in `crates/merman-export/tests/platform_font_context.rs` verifies the
tags, outline geometry, advances and text mappings of two instances on one page.

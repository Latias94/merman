# Changelog

The Typst package has its own version axis; its bundled Merman version is recorded separately.

## [0.5.0] - Unreleased

This entry prepares the wrapper bundling Merman 0.9.0 relative to published 0.4.0. It does not announce Typst Universe availability. Plugin ABI 2 and minimum Typst 0.15.0 remain unchanged.

### Changed

- SVG leaves the canvas background unset. Use `background: "white"` when opaque artwork is needed; the surrounding Typst page controls presentation otherwise. [PR #180](https://github.com/Latias94/merman/pull/180)
- Deep diagram ownership and cleanup are safer across valid, malformed, cancelled, and resource-rejected input, while the wrapper's Mermaid JSON and plugin ABI remain compatible. [PR #191](https://github.com/Latias94/merman/pull/191)

### Fixed

- Unicode HTML labels wrap hyphens, slashes, soft hyphens, and whitespace consistently without losing decoded label text. [PR #192](https://github.com/Latias94/merman/pull/192), [PR #193](https://github.com/Latias94/merman/pull/193)
- Class and ER hollow relationship markers remain hollow in resvg-safe output. [PR #182](https://github.com/Latias94/merman/pull/182)
- Block children reflect later metadata updates and repeated composite declarations retain their first child list and columns. [PR #191](https://github.com/Latias94/merman/pull/191)

## [0.4.0] - 2026-10-09

Published on Typst Universe, bundling Merman 0.8.0 with Mermaid 12.1 behavior, plugin ABI 2, and minimum Typst 0.15.0. See the [dated publication record](../../../docs/release/PUBLISH_ORDER.md#typst-040-publication). The immutable 0.4.0 package is not rewritten by later source preparation.

# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [0.8.0-alpha.5] - 2026-08-11

### Added

- Added the first experimental public alpha of `@mermanjs/node` for Node.js 22 and newer on macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC. The loader installs one exact-version native package and provides deterministic SVG rendering plus metadata/layout operations without lifecycle downloads or a browser-WASM fallback.

### Known limitations

- The distributed native recipe includes SVG, Cytoscape, and ELK only. Math, analysis, ASCII, PNG, JPEG, PDF, host text measurement, and native runtime adapters are not part of this package group; use the runtime catalog and typed missing-capability errors instead of assuming an operation is present.

# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [0.8.0-alpha.6] - 2026-09-02

This section describes the prepared alpha.6 source candidate. The npm alpha channel remains independently published and may still resolve alpha.5 until the alpha.6 package group is authorized.

### Added

- Added the opt-in `@mermanjs/node-wasm` package with a Node-targeted wasm-bindgen artifact. It is published separately from the native `@mermanjs/node` loader and never reuses `@mermanjs/web`.
- Added a typed `MermanNativeLoadError` for installed native packages that fail dynamic loading, including ABI and glibc diagnostics.
- Moved Linux GNU candidate builds to a glibc 2.31 baseline container and recorded the build environment in the candidate receipt.

## [0.8.0-alpha.5] - 2026-08-11

### Added

- Added the first experimental public alpha of `@mermanjs/node` for Node.js 22 and newer on macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC. The loader installs one exact-version native package and provides deterministic SVG rendering plus metadata/layout operations without lifecycle downloads or a browser-WASM fallback.

### Known limitations

- The distributed native recipe includes SVG, Cytoscape, and ELK only. Math, analysis, ASCII, PNG, JPEG, PDF, host text measurement, and native runtime adapters are not part of this package group; use the runtime catalog and typed missing-capability errors instead of assuming an operation is present.
- The immutable `@mermanjs/node@0.8.0-alpha.5` loader tarball was packed before this heading was dated and therefore contains an `Unreleased` heading. This documentation-only bootstrap defect is corrected in source and will first appear in a later package version.

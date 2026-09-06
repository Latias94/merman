# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [Unreleased]

### Added

- Added asynchronous `renderDrawingList(...)` and synchronous `renderDrawingListSync(...)` APIs to the native and Node-targeted WASM transports. Both transports advertise the shared `drawing-list-json` operation and return validated DrawingList v1 JSON without selecting an SVG fallback.

## [0.8.0-alpha.6] - 2026-09-02

This section describes the published alpha.6 package group. All seven packages, including `@mermanjs/node-wasm`, were manually bootstrapped from the verified package-group artifact. The immutable alpha.6 npm tarballs have no npm provenance; later releases use Trusted Publishing.

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

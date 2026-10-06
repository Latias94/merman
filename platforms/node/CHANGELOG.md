# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [0.8.0] - 2026-10-06

This entry summarizes the package since its introduction during the 0.8 alphas; there was no 0.7.0 Node package. All seven packages were published on npm at 0.8.0 on 2026-10-06. The package group remains experimental and requires Node.js 22 or newer.

### Added

- Choose native `@mermanjs/node` with its exact-version platform package or the explicit `@mermanjs/node-wasm` transport. Both provide deterministic SVG/layout operations; neither silently switches transport or uses browser WASM as a fallback.
- The current diagram surface includes Agentflow and Usecase. Agentflow follows upstream beta syntax; the package does not add ASCII, analysis, math, binary export, or host text-measurement callbacks.

### Upgrade

- Keep the loader and selected transport artifacts on one package version. Native load failures carry typed ABI/platform diagnostics; resolve these rather than substituting a browser package.
- Relative to early 0.8 alphas, this release selects Mermaid 12.1.0 with Mermaid 12 ELK and Redux/Neo defaults. Review SVG snapshots and explicit top-level layout/theme/look settings. Node rendering is not browser DOM admission.

### Fixed

- Corrected C4 and ELK relationship routing/layering, Packet/XYChart presentation, partial theme overrides, and diagnostic locations; restored Usecase/ER styles and prevented malformed Unicode colors from panicking.

### Further reading

- Use the [package guide](README.md), [stable migration guide](../../docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels), and [root changelog](../../CHANGELOG.md) for the published contract. Native Rust benchmark observations are not Node transport measurements.

## [0.8.0-alpha.7] - 2026-09-30

### Added

- Added Mermaid 12 Agentflow and Usecase parsing and SVG rendering. Agentflow follows upstream beta syntax; neither family supports ASCII output.

### Changed

- Updated the engine from Mermaid 11.17.2 to 12.0.0, including ELK layout defaults and the new theme/look defaults for supported families. Refresh SVG snapshots; use top-level `layout: dagre`, `theme: default`, and `look: classic` when the previous presentation is needed. The bundled artifact already included ELK and retains its EPL-2.0 notices.
- Safe diagram-local theme variables and fonts are admitted from frontmatter and directives while host security policy remains authoritative.

### Compatibility

- Native and Node-targeted WASM packages remain an experimental Node.js 22+ group. The existing SVG/layout capability recipe does not add ASCII, analysis, math, or raster/PDF export. Update the loader and all selected transport packages together.

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

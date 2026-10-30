# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [0.9.3] - 2026-10-30

- Sequence diagrams and GitGraph tag backgrounds resolve their diagram-scoped built-in Neo shadows. Sequence viewports include box-frame strokes and restored built-in Neo shadows to prevent clipping. [#255](https://github.com/Latias94/merman/pull/255)

- Flowchart preserves percent-containing node IDs, and YAML frontmatter no longer mistakes brackets in comments or block text for nested collections. [#241](https://github.com/Latias94/merman/pull/241), [#251](https://github.com/Latias94/merman/pull/251)
- `createNodeEngine({ iconPacks })` accepts already-loaded Iconify JSON for icons scoped to that engine. The application owns loading and credentials. [#247](https://github.com/Latias94/merman/pull/247)
- Correct label sizing, configured fonts, literal text, and HTML/entity handling across SVG diagrams, including wrapped Swimlane edge labels and valid Class-note XML. Review updated output snapshots. [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247), [#250](https://github.com/Latias94/merman/pull/250)
- Apply Gantt `tickInterval`, Mindmap root padding and width settings, and Sequence numbering, participant visibility, and note alignment. Init directives follow Mermaid 12.1.0's filtering and merge order. [#252](https://github.com/Latias94/merman/pull/252)
- Refresh the native and WASM dependency graphs while retaining Mermaid 12.1.0, Options JSON schema 2, Node transport 1, and text-measurement protocol 1 with its 22 operations (0..21). The host literal-height extension (operation 22) is deferred; see the [host guide](../../docs/bindings/HOST_TEXT_MEASUREMENT.md) for callback limits. Native C/WASI SDK archives have separate APIs and capabilities; this package continues to provide its own SVG recipe. See the [workspace changelog](../../CHANGELOG.md). [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247)

## [0.9.2] - 2026-10-22

- Host themes keep C4 boundary contents and relationships visible, color Wardley anchors, and apply a coherent ZenUML palette. Diagram-selected Mermaid palettes retain matching text and fills. [#235](https://github.com/Latias94/merman/pull/235), [issue #229](https://github.com/Latias94/merman/issues/229), [issue #230](https://github.com/Latias94/merman/issues/230), [issue #231](https://github.com/Latias94/merman/issues/231), [issue #232](https://github.com/Latias94/merman/issues/232)
- Look selectors and Block/Requirement source CSS are validated before SVG emission. SVG CSS cleanup rejects unterminated strings, comments, and URLs while preserving safe neighboring declarations. [#227](https://github.com/Latias94/merman/pull/227), [#235](https://github.com/Latias94/merman/pull/235), [#237](https://github.com/Latias94/merman/pull/237)
- Repeated Flowchart `classDef` declarations retain earlier styles, and unsupported Unicode characters preserve valid error spans and lexer progress. Malformed Pie/Info headers and empty XYChart sources behave consistently across parser and editor entrypoints. [#178](https://github.com/Latias94/merman/pull/178), [#227](https://github.com/Latias94/merman/pull/227), [#237](https://github.com/Latias94/merman/pull/237)

The existing Presentation and binding options contracts are unchanged. Review regenerated SVG and exported-image snapshots; see the [workspace changelog](../../CHANGELOG.md) for the complete maintenance release.

## [0.9.1] - 2026-10-18

- ER diagram SVG emission reuses prepared entity geometry, avoiding false missing-size errors on larger diagrams.
- Diagram lexers now make progress and honor cancellation while skipping malformed or empty input.
- Generated HTML label IDs are scoped to the diagram root so multiple diagrams can share a document.

## [0.9.0] - 2026-10-12

The experimental `0.9.0` package group requires Node.js 22 or newer. Keep the loader and selected native/WASM transport artifacts at the same version.

### Added

- SVG-capable operations add `edge-geometry-json` for final clipped points and painted label centers in Flowchart, Swimlane, and Agentflow. `layout-json` remains a pre-paint projection; query the loaded operation catalog before use. Thanks [@isaced](https://github.com/isaced) for the contribution. [PR #174](https://github.com/Latias94/merman/pull/174), [PR #177](https://github.com/Latias94/merman/pull/177)

### Changed

- SVG leaves the canvas unset. Supply an explicit SVG output background when the host needs opaque artwork. [PR #180](https://github.com/Latias94/merman/pull/180)

### Fixed

- Unicode HTML wrapping preserves complete decoded labels and whitespace; resvg-safe Class/ER markers stay hollow. [PR #182](https://github.com/Latias94/merman/pull/182), [PR #192](https://github.com/Latias94/merman/pull/192)
- Deep cleanup and Block metadata updates follow the shared engine contract without changing transport selection. Native CLI font discovery is not a Node host-measurement feature. [PR #191](https://github.com/Latias94/merman/pull/191)

See the [package guide](README.md) and [migration guide](../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md); Rust CLI measurements do not measure Node transport latency.

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

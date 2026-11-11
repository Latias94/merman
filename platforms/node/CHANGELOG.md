# Changelog

All notable changes to the `@mermanjs/node` package group will be documented in this file.

## [0.10.0] - Unreleased

Cumulative changes since the latest stable Node package group, `0.9.3`. The package remains experimental; see the [stable upgrade guide](../../docs/release/V093_TO_V0100_UPGRADE_GUIDE.md) for the complete migration.

| Upgrading from | Next step |
| --- | --- |
| `0.9.3` | Apply the breaking changes below and refresh SVG snapshots. |
| `0.10.0-alpha.1` or `alpha.2` | Review the guide's remaining contract and theme-catalog changes. |

### Breaking changes

- Keep the nine-package group on one exact release: `@mermanjs/node`, its seven native platform packages, and explicit `@mermanjs/node-wasm`. Custom facades and native/WASM artifacts must come from the same source revision. Node binding API remains `1`; browser Web packages are a separate product.
- Migrate Options JSON from schema `2` to `3`: replace `presentation` styling with top-level `theme` and remove retired presentation-profile and host-owned CSS/security fields. Replace `metadataJson("presentation-catalog")` with `metadataJson("theme-catalog")`.
- Update custom render-metadata decoders for `theme_execution_evidence.version: 2` and required nullable `materialized_theme`. Keep unknown evidence versions opaque and inspect output-target admission separately from theme application. [#277](https://github.com/Latias94/merman/pull/277)
- Handle formula rejection under `constrained`, or choose `interactive` or an appropriate trusted policy with host execution and memory limits. Installing a math-capable transport does not relax resource admission.

### Added

- Apply optional themes from 11 design groups with 14 concrete presets, including `paper-cut`, `carbon-copy`, and `pop-art`, or import editable JSON. Core colors and fonts reach all 34 diagram families, respecting source-owned styles and the typography used to measure labels. Presets retain alpha maturity; effects and output support depend on the family, shape, look, and target. See the [theme guide](../../docs/rendering/custom-diagram-themes.md) and [coverage limits](../../docs/rendering/diagram-theme-coverage.md). [#178](https://github.com/Latias94/merman/pull/178), [#277](https://github.com/Latias94/merman/pull/277), [#299](https://github.com/Latias94/merman/pull/299)
- Keep light and dark appearances in one schema-1 recipe. Editor, Gruvbox, and Ayu are paired designs; catalog consumers read `appearances` and each `concrete_preset_id`. Export the complete recipe with `export-theme-preset-json`, inspect or materialize it with `materialize-theme-json`, and resolve Follow-app policy to a declared appearance in the host. Whole-recipe validation includes inactive appearances. [#284](https://github.com/Latias94/merman/pull/284)
- Use native GNU or musl Linux ARM64 alongside the existing targets; GNU Linux requires glibc `2.31+`. Both native Node and explicit Node-WASM include SVG math, Cytoscape, and ELK. Binary export, analysis, ASCII, and text-measurement callbacks remain outside this SVG product. See the [package guide](README.md). [#265](https://github.com/Latias94/merman/pull/265), [#266](https://github.com/Latias94/merman/pull/266)

### Changed and fixed

- Refine Cyberpunk with clear neutral text and restrained neon contours, and Ghibli with warm paper, sage surfaces and independent chart inks. Both add dedicated Flowchart, Sequence and XY Chart styling while retaining editable recipe export.
- Advance from Mermaid `12.1.0` to `12.3.0`, including timed Flowchart Step edges, numbered badges, nine `bpmn-*` shapes, and updated Swimlane and Usecase behavior. Static SVG retains Step edges and badges without browser animation. Review snapshots for ELK `0.12.1` routing, label clearance, containers, and packing; see the [Mermaid migration](../../docs/migrations/mermaid-12-3.md). [#277](https://github.com/Latias94/merman/pull/277)
- Correct Class, Sequence, and State source handling and label geometry, including rich Class relationship labels and Sequence frames. Review affected SVG snapshots. [#282](https://github.com/Latias94/merman/pull/282), [#286](https://github.com/Latias94/merman/pull/286), [#294](https://github.com/Latias94/merman/pull/294), [#297](https://github.com/Latias94/merman/pull/297)
- Enforce JSON byte, member, token, and string limits consistently in native and Node-WASM engine construction, including defaults and shared values, without mutating caller input or invoking getters or `toJSON`. [#277](https://github.com/Latias94/merman/pull/277)

## [0.10.0-alpha.2] - 2026-11-06

Changes since `0.10.0-alpha.1`. See the [host installation guide](../../docs/release/PREBUILT_PRODUCTS.md) for independently verified package availability.

### Breaking changes

- Upgrade the loader and selected native or Node-targeted WASM package together to the same exact version. Custom builds must pair their JavaScript facade and native/WASM artifact from the same source revision; browser `@mermanjs/web` packages are not Node transport substitutes.
- Custom render-metadata decoders must handle `theme_execution_evidence.version: 2` and its required nullable `materialized_theme`, which records the Mermaid palette actually used by the render. Keep unknown evidence versions opaque and inspect target admission separately from successful theme application. Options JSON remains schema `3`; see the [contract migration](../../docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up). [#277](https://github.com/Latias94/merman/pull/277)

### Changed

- Advance Mermaid compatibility from `12.2.0` to `12.3.0`, adding timed Flowchart Step edges and numbered badges, nine `bpmn-*` Flowchart shapes, updated Swimlane behavior and Usecase Include/Extend labels. Step animation is browser-only; static output keeps ordinary edges and badges. ELK follows the selected `0.12.1` behavior for label clearance, compound containers, self-loops and packing, so review SVG/layout snapshots after upgrading. See the [Mermaid migration guide](../../docs/migrations/mermaid-12-3.md). [#277](https://github.com/Latias94/merman/pull/277)
- Core theme colors and typography reach all 34 diagram families in the complete SVG product, including supported paint on BPMN event rings and annotation brackets. Shadows, glows and other effects retain their family, shape and look restrictions; successful rendering does not certify output portability. See the [theme coverage guide](../../docs/rendering/diagram-theme-coverage.md). [#277](https://github.com/Latias94/merman/pull/277), [#278](https://github.com/Latias94/merman/pull/278)
- Reuse 15 editable designs from the [theme design gallery](../../CHANGELOG.md#theme-design-gallery), with dedicated Flowchart, Sequence, Class, XY Chart and Pie examples. These are independent JSON recipes, not additions to the 11 built-in presets. Pass a downloaded recipe as the `theme` value in [Options JSON](../../docs/bindings/OPTIONS_JSON.md#diagram-theme); fonts and other diagram/output combinations need their own review. [#279](https://github.com/Latias94/merman/pull/279)

### Fixed

- Apply JSON byte, member, token and string limits consistently when constructing native and Node WASM engines, including inserted defaults and repeated shared values, without mutating caller input or executing getters and `toJSON` hooks. [#277](https://github.com/Latias94/merman/pull/277)
- Preserve Flowchart source-class precedence over theme colors and fonts, and use supported source font sizes during label measurement. [#278](https://github.com/Latias94/merman/pull/278)
- Avoid false strict-theme rejections when resetting supported Pie text styles or Architecture group colors to their defaults. [#278](https://github.com/Latias94/merman/pull/278)
- Preserve supported inline font sizes, weights and code fonts during HTML-to-SVG conversion; retain Railroad, Cynefin and Wardley label text, and emit valid SVG for State labels with HTML line breaks. [#277](https://github.com/Latias94/merman/pull/277), [#278](https://github.com/Latias94/merman/pull/278)
- Render Brutalist shadows on supported Classic and Neo Flowchart shapes and match Usecase arrowheads to connector colors. Explicit Sequence theme backgrounds now contain the full lifeline when bottom participants are hidden. [#278](https://github.com/Latias94/merman/pull/278), [#279](https://github.com/Latias94/merman/pull/279)

## [0.10.0-alpha.1] - 2026-11-02

Changes below are relative to `0.9.3`. Install the exact prerelease with `npm install --save-exact @mermanjs/node@0.10.0-alpha.1`, or explicitly select `@mermanjs/node-wasm@0.10.0-alpha.1`; see the [host installation guide](../../docs/release/PREBUILT_PRODUCTS.md).

### Breaking changes

- Upgrade the loader and selected native or WASM transport packages together. Replace `metadataJson("presentation-catalog")` with `metadataJson("theme-catalog")`.
- Update saved Options JSON from schema `2` to `3` in both transports: replace `presentation` styling inputs with top-level `theme: {preset: ...}` or `theme: {spec: ...}`, and use `raster.matte` and `pdf.page_paint` for export backgrounds. Removed presentation-profile and host-owned CSS/security fields are rejected.
- For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes` and remove SVG-element/backend overrides. The geometry payload remains schema `1`. See the [migration guide](../../docs/migrations/edge-geometry-resources.md).
- Formulas are rejected under `constrained` in both transports even when math is installed. Handle that rejection, or select `interactive` or an appropriate trusted policy for permitted workloads with host execution and memory limits.

### Added

- Install `@mermanjs/node` on GNU or musl Linux ARM64 as well as the existing native targets. Installing the root package selects the matching exact-version optional dependency. Native GNU Linux builds require glibc 2.31 or newer. See the [package guide](README.md). [#265](https://github.com/Latias94/merman/pull/265), [#266](https://github.com/Latias94/merman/pull/266)
- Render formulas with both native `@mermanjs/node` and explicit `@mermanjs/node-wasm` SVG products; no custom math-enabled build is needed.
- Apply 11 alpha theme presets or share your own editable JSON recipe. `metadataJson("theme-catalog")` lists presets and their intended diagram families; theme authoring operations export recipes and query styling support for your diagram and output. See the [theme guide and gallery](../../docs/rendering/custom-diagram-themes.md) for examples and alpha limitations. [#178](https://github.com/Latias94/merman/pull/178), [#223](https://github.com/Latias94/merman/pull/223), [#234](https://github.com/Latias94/merman/pull/234), [#253](https://github.com/Latias94/merman/pull/253)

## [0.9.3] - 2026-10-30

These fixes are included in the published `0.9.3` npm package group.

- Use loaded Iconify packs with `createNodeEngine({ iconPacks })`; icons are scoped to that engine. [#247](https://github.com/Latias94/merman/pull/247)
- Fix clipped labels, font sizing, literal text, and HTML/entity handling, including wrapped Swimlane edge labels and valid Class-note XML. [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247), [#250](https://github.com/Latias94/merman/pull/250)
- Restore Neo shadows in Sequence diagrams and GitGraph tag backgrounds, and prevent Sequence boxes from clipping at the diagram edges. [#255](https://github.com/Latias94/merman/pull/255)
- Honor Gantt `tickInterval`, Mindmap root padding/width, and Sequence numbering, participant visibility, and note alignment. When combining init directives, replace legacy `config` with explicit namespaces or frontmatter. [#252](https://github.com/Latias94/merman/pull/252)
- Preserve `%` in Flowchart node IDs and accept brackets in YAML frontmatter comments and block text. Separate Flowchart header directions from the diagram body with a newline or semicolon. [#241](https://github.com/Latias94/merman/pull/241), [#251](https://github.com/Latias94/merman/pull/251)

Review SVG snapshots after upgrading. See the [workspace changelog](../../CHANGELOG.md) for the complete release.

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

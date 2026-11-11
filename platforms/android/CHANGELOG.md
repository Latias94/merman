# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.10.0] - 2026-11-12

Cumulative changes since the latest stable Android package, `0.9.3`. See the [stable upgrade guide](../../docs/release/V093_TO_V0100_UPGRADE_GUIDE.md) for the complete migration.

| Upgrading from | Next step |
| --- | --- |
| `0.9.3` | Apply the breaking changes below and refresh output snapshots. |
| `0.10.0-alpha.1` or `alpha.2` | Review the guide's remaining contract and theme-catalog changes. |

### Breaking changes

- Upgrade the AAR as a unit so Kotlin classes and `libmerman_android_jni.so` come from the same build. Custom builds must regenerate the Kotlin artifact contract with the native libraries; unchanged transport versions do not establish JSON compatibility.
- Migrate Options JSON from schema `2` to `3`: replace `presentation` styling with top-level `theme`, use `raster.matte` and `pdf.page_paint` for custom export backgrounds, and remove retired presentation-profile and host-owned CSS/security fields. Replace `presentationCatalogJson()` with `themeCatalogJson()`.
- Update analysis-facts decoders from schema `2` to `3` for required per-diagram `appearance` declarations with source provenance and UTF-8 byte spans. Render metadata separately uses `theme_execution_evidence.version: 2` with required nullable `materialized_theme`; preserve unknown evidence versions as opaque data. [#277](https://github.com/Latias94/merman/pull/277)
- Custom text measurers must handle `html-line-height` (operation `22`) with a length result or leave it unhandled for fallback. Text-measurement protocol remains `1`. [#249](https://github.com/Latias94/merman/pull/249)
- For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes` and remove SVG-element/backend overrides. The geometry payload remains schema `1`; see the [resource migration](../../docs/migrations/edge-geometry-resources.md).
- Handle formula rejection under `constrained`, or choose `interactive` or an appropriate trusted policy with host execution and memory limits. Installing a math-capable AAR does not relax resource admission.

### Added

- Apply optional themes from 11 design groups with 14 concrete presets, including `paper-cut`, `carbon-copy`, and `pop-art`, or import editable JSON. Core colors and fonts reach all 34 diagram families, respecting source-owned styles and the typography used to measure labels. Presets retain alpha maturity; effects and output support depend on the family, shape, look, and target. See the [theme guide](../../docs/rendering/custom-diagram-themes.md) and [coverage limits](../../docs/rendering/diagram-theme-coverage.md). [#178](https://github.com/Latias94/merman/pull/178), [#277](https://github.com/Latias94/merman/pull/277), [#299](https://github.com/Latias94/merman/pull/299)
- Keep light and dark appearances in one schema-1 recipe. Editor, Gruvbox, and Ayu are paired designs; catalog consumers read `appearances` and each `concrete_preset_id`. Use `export-theme-preset-json` and `materialize-theme-json` through `execute()` to retain editable recipes and validate every appearance. Resolve Follow-app policy in Kotlin before selecting `light` or `dark`. [#284](https://github.com/Latias94/merman/pull/284)
- Resolve the AAR and transitive dependencies from a complete selected-version Maven repository archive. The ordinary AAR includes SVG math, Cytoscape/ELK, ASCII, and analysis, while PNG/JPEG/PDF encoders and system font discovery remain absent. Maven Central remains a separate publication channel; use the [local Maven guide](README.md#resolve-a-candidate-maven-publication). [#265](https://github.com/Latias94/merman/pull/265), [#266](https://github.com/Latias94/merman/pull/266)

### Changed and fixed

- Refine Cyberpunk with clear neutral text and restrained neon contours, and Ghibli with warm paper, sage surfaces and independent chart inks. Both add dedicated Flowchart, Sequence and XY Chart styling while retaining editable recipe export. [#302](https://github.com/Latias94/merman/pull/302)
- Advance from Mermaid `12.1.0` to `12.3.0`, including timed Flowchart Step edges, numbered badges, nine `bpmn-*` shapes, and updated Swimlane and Usecase behavior. Static output retains Step edges and badges without browser animation. Review snapshots for ELK `0.12.1` routing, label clearance, containers, and packing; see the [Mermaid migration](../../docs/migrations/mermaid-12-3.md). [#277](https://github.com/Latias94/merman/pull/277)
- Correct Class, Sequence, and State source handling and label geometry; semantic/layout JSON preserves the selected Mermaid numeric behavior. ASCII Flowchart feedback edges route around occupied geometry, and Sequence accepts `and` sections in `par_over`. [#282](https://github.com/Latias94/merman/pull/282), [#286](https://github.com/Latias94/merman/pull/286), [#287](https://github.com/Latias94/merman/pull/287), [#294](https://github.com/Latias94/merman/pull/294), [#297](https://github.com/Latias94/merman/pull/297)

## [0.10.0-alpha.2] - 2026-11-06

Changes since `0.10.0-alpha.1`. See the [host installation guide](../../docs/release/PREBUILT_PRODUCTS.md) for independently verified package availability.

### Breaking changes

- Upgrade the AAR as a unit. Custom builds must regenerate matching Kotlin helpers and native libraries from the same source revision; Options JSON remains schema `3` while the JSON contracts below change.
- Update analysis-facts decoders from schema `2` to `3`. Every diagram requires an `appearance` sibling of `syntax`, tagged `known` or `unknown`; known theme, font and look declarations include frontmatter/directive provenance and original diagram-local UTF-8 byte spans. Source declarations describe configuration, not the executed render. See the [contract migration](../../docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up). [#277](https://github.com/Latias94/merman/pull/277)
- Update custom render-metadata decoders for `theme_execution_evidence.version: 2` and required nullable `materialized_theme`, which records the Mermaid palette actually used. Preserve unknown evidence versions as opaque data; theme application and output-target admission remain separate checks. [#277](https://github.com/Latias94/merman/pull/277)

### Changed

- Advance Mermaid compatibility from `12.2.0` to `12.3.0`, adding timed Flowchart Step edges and numbered badges, nine `bpmn-*` Flowchart shapes, updated Swimlane behavior and Usecase Include/Extend labels. Animation is browser-only; static output keeps edges and badges. Review SVG/layout snapshots for the selected ELK `0.12.1` label clearance, compound-container, self-loop and packing behavior. See the [Mermaid migration guide](../../docs/migrations/mermaid-12-3.md). [#277](https://github.com/Latias94/merman/pull/277)
- Core theme colors and typography reach all 34 diagram families in the complete SVG product, including supported paint on BPMN event rings and annotation brackets. Effects remain specific to the family, shape, look and output; see the [theme coverage guide](../../docs/rendering/diagram-theme-coverage.md). [#277](https://github.com/Latias94/merman/pull/277), [#278](https://github.com/Latias94/merman/pull/278)
- Adapt 15 editable designs from the [theme design gallery](../../CHANGELOG.md#theme-design-gallery) by passing the downloaded JSON as the `theme` value in [Options JSON](../../docs/bindings/OPTIONS_JSON.md#diagram-theme). These five-family examples are independent recipes; the built-in catalog remains at 11 presets. Fonts resolve on the host, and examples do not qualify every diagram or output. [#279](https://github.com/Latias94/merman/pull/279)

### Fixed

- Preserve Flowchart source-class precedence over theme colors and fonts, and use supported source font sizes during label measurement. [#278](https://github.com/Latias94/merman/pull/278)
- Avoid false strict-theme rejections when resetting supported Pie text styles or Architecture group colors to their defaults. [#278](https://github.com/Latias94/merman/pull/278)
- Preserve supported inline font sizes, weights and code fonts during HTML-to-SVG conversion; retain Railroad, Cynefin and Wardley label text, and emit valid SVG for State labels with HTML line breaks. [#277](https://github.com/Latias94/merman/pull/277), [#278](https://github.com/Latias94/merman/pull/278)
- Render Brutalist shadows on supported Classic and Neo Flowchart shapes and match Usecase arrowheads to connector colors. Explicit Sequence theme backgrounds now contain the full lifeline when bottom participants are hidden. [#278](https://github.com/Latias94/merman/pull/278), [#279](https://github.com/Latias94/merman/pull/279)

## [0.10.0-alpha.1] - 2026-11-02

Changes below are relative to `0.9.3`. Download the matching AAR or local Maven package using the [host installation guide](../../docs/release/PREBUILT_PRODUCTS.md).

### Breaking changes

- Upgrade the AAR as a unit; custom builds must regenerate matching Kotlin helpers and native libraries. Replace `presentationCatalogJson()` with `themeCatalogJson()`.
- Custom text measurers must handle `html-line-height` (operation `22`) with a length result or leave it unhandled for fallback measurement. Text-measurement protocol `1` is unchanged; see the [callback contract](../../docs/bindings/HOST_TEXT_MEASUREMENT.md#text-measurement-protocol-summary). [#249](https://github.com/Latias94/merman/pull/249)
- Update saved Options JSON from schema `2` to `3`: replace `presentation` styling inputs with top-level `theme: {preset: ...}` or `theme: {spec: ...}`, and use `raster.matte` and `pdf.page_paint` for export backgrounds. Removed presentation-profile and host-owned CSS/security fields are rejected.
- For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes` and remove SVG-element/backend overrides. The geometry payload remains schema `1`. See the [migration guide](../../docs/migrations/edge-geometry-resources.md).
- Formulas are rejected under `constrained` even when math is installed. Handle that rejection, or select `interactive` or an appropriate trusted policy for permitted workloads with host execution and memory limits.

### Added

- Resolve the AAR and its dependencies from a complete, selected-version Maven repository archive. See the [Maven installation guide](README.md#resolve-a-candidate-maven-publication). [#265](https://github.com/Latias94/merman/pull/265), [#266](https://github.com/Latias94/merman/pull/266)
- Render formulas with the ordinary SVG product; no custom math-enabled build is needed. Image encoders and system font discovery remain outside this product.
- Apply 11 alpha theme presets or share your own editable JSON recipe. `themeCatalogJson()` lists presets and their intended diagram families; theme authoring operations export recipes and query styling support for your diagram and output. See the [theme guide and gallery](../../docs/rendering/custom-diagram-themes.md) for examples and alpha limitations. [#178](https://github.com/Latias94/merman/pull/178), [#223](https://github.com/Latias94/merman/pull/223), [#234](https://github.com/Latias94/merman/pull/234), [#253](https://github.com/Latias94/merman/pull/253)

## [0.9.3] - 2026-10-30

The `0.9.3` AAR is available on the [v0.9.3 Release](https://github.com/Latias94/merman/releases/tag/v0.9.3).

- Fix clipped labels, font sizing, literal text, and HTML/entity handling, including wrapped Swimlane edge labels and valid Class-note XML. [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247), [#250](https://github.com/Latias94/merman/pull/250)
- Restore Neo shadows in Sequence diagrams and GitGraph tag backgrounds, and prevent Sequence boxes from clipping at the diagram edges. [#255](https://github.com/Latias94/merman/pull/255)
- Honor Gantt `tickInterval`, Mindmap root padding/width, and Sequence numbering, participant visibility, and note alignment. When combining init directives, replace legacy `config` with explicit namespaces or frontmatter. [#252](https://github.com/Latias94/merman/pull/252)
- Preserve `%` in Flowchart node IDs and accept brackets in YAML frontmatter comments and block text. Separate Flowchart header directions from the diagram body with a newline or semicolon. [#241](https://github.com/Latias94/merman/pull/241), [#251](https://github.com/Latias94/merman/pull/251)

Review SVG snapshots after upgrading. See the [workspace changelog](../../CHANGELOG.md) for the complete release.

## [0.9.2] - 2026-10-22

- Host themes keep C4 boundary contents and relationships visible, color Wardley anchors, and apply a coherent ZenUML palette. Diagram-selected Mermaid palettes retain matching text and fills. [#235](https://github.com/Latias94/merman/pull/235), [issue #229](https://github.com/Latias94/merman/issues/229), [issue #230](https://github.com/Latias94/merman/issues/230), [issue #231](https://github.com/Latias94/merman/issues/231), [issue #232](https://github.com/Latias94/merman/issues/232)
- Custom artifacts with native font support skip symbolic and pictorial fonts during implicit font and missing-glyph fallback; explicitly named fonts remain selectable. [#235](https://github.com/Latias94/merman/pull/235), [issue #228](https://github.com/Latias94/merman/issues/228)
- Look selectors and Block/Requirement source CSS are validated before SVG emission. SVG CSS cleanup rejects unterminated strings, comments, and URLs while preserving safe neighboring declarations. [#227](https://github.com/Latias94/merman/pull/227), [#235](https://github.com/Latias94/merman/pull/235), [#237](https://github.com/Latias94/merman/pull/237)
- Repeated Flowchart `classDef` declarations retain earlier styles, and unsupported Unicode characters preserve valid error spans and lexer progress. Malformed Pie/Info headers and empty XYChart sources behave consistently across parser and editor entrypoints. [#178](https://github.com/Latias94/merman/pull/178), [#227](https://github.com/Latias94/merman/pull/227), [#237](https://github.com/Latias94/merman/pull/237)

The existing Presentation and binding options contracts are unchanged. Review regenerated SVG and exported-image snapshots; see the [workspace changelog](../../CHANGELOG.md) for the complete maintenance release.

## [0.9.1] - 2026-10-18

- ER diagram SVG emission reuses prepared entity geometry, avoiding false missing-size errors on larger diagrams.
- Diagram lexers now make progress and honor cancellation while skipping malformed or empty input.
- Generated HTML label IDs are scoped to the diagram root so multiple diagrams can share a document.

## [0.9.0] - 2026-10-12

Version `0.9.0` retains Android JNI API 2 from 0.8.0. Keep the generated wrapper and native artifacts paired.

### Added

- SVG-capable operations add `edge-geometry-json` for Flowchart, Swimlane, and Agentflow clipped edge points and painted label centers. `layout-json` keeps its pre-paint coordinates; inspect the installed operation catalog. [PR #174](https://github.com/Latias94/merman/pull/174), [PR #177](https://github.com/Latias94/merman/pull/177)
- Flowchart, Swimlane, and State ASCII/Unicode node padding accepts independent X/Y overrides, including zero, separately from graph spacing. [PR #188](https://github.com/Latias94/merman/pull/188)

### Upgrade

- Host text-measurement protocol 1 adds signed raw bounding-box Y-offset operations 19–21. Update exhaustive callback handlers without changing the protocol number. Native CLI font flags do not enable native font discovery in the default package. [PR #193](https://github.com/Latias94/merman/pull/193)
- Review transparent SVG backgrounds and corrected geometry/wrapping snapshots. Set `svg.root_background_color` through the binding options when opaque SVG is required; match the installed runtime catalog rather than assuming every optional operation is compiled.

### Fixed

- Corrected Unicode HTML wrapping and resvg-safe hollow Class/ER markers; SVG leaves the canvas unset unless an explicit background is requested. [PR #180](https://github.com/Latias94/merman/pull/180), [PR #182](https://github.com/Latias94/merman/pull/182), [PR #192](https://github.com/Latias94/merman/pull/192)
- Deep input cleanup no longer relies on recursive ownership. Block child metadata and repeated composite declarations follow the pinned Mermaid behavior. [PR #191](https://github.com/Latias94/merman/pull/191)
- ASCII/Unicode output preserves loop-back contacts, transition labels, compound frames, Sequence spacing, and diamond shoulders. Resource ceilings retain their existing meanings. [PR #179](https://github.com/Latias94/merman/pull/179), [PR #188](https://github.com/Latias94/merman/pull/188)

See the [package guide](README.md) and [upgrade guide](../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md#binding-artifacts-and-callbacks).

## [0.8.0] - 2026-10-06

This entry consolidates the previous stable integration and the 0.8 alpha migrations into the final candidate contract. It does not announce AAR publication or a Maven Central release.

### Breaking changes

- Replace the C-ABI-forwarding JNI bridge with direct JNI transport API 2. Ship the matching Kotlin classes and `libmerman_android_jni.so` slices in one AAR; old `libmerman_ffi.so` JNI slices are incompatible.
- Replace `MermanReusableEngine` and mutable `setTextMeasurer()` calls with `MermanEngine(optionsJson, services)` and immutable constructor services. Callback-enabled engines reject concurrent/reentrant calls with typed errors; retry `close()` when an active operation prevents closure.
- Adopt Options JSON schema 2, analysis facts schema 2, and ASCII report schema 3. Rename viewport fields to `container_width` / `container_height`; update requested/effective layout, encoding, and structured-text fallback decoders independently from the JNI API version.

### Added

- Added reusable generic operations, cooperative cancellation/deadlines, runtime catalogs, typed diagnostic/resource details, and richer ASCII/viewport controls.

### Changed

- The selected engine advances from Mermaid 11.15.0 in the previous stable line to the 12.1.0 candidate. Review ELK layout defaults, Redux/Neo appearance, SVG IDs, and geometry; request top-level `layout: dagre`, `theme: default`, and `look: classic` when those presentation choices are required. This does not preserve every old SVG byte.
- Default native artifacts include SVG, Cytoscape/ELK, ASCII, analysis, validation, and document analysis. Math, PNG/JPEG/PDF, and native runtime adapters require a matching custom artifact; query the loaded runtime catalog before using optional operations.

### Fixed

- Corrected C4 and ELK relationship routing/layering, Packet/XYChart presentation, partial theme overrides, and diagnostic locations; restored Usecase/ER styles and prevented malformed Unicode colors from panicking.

### Further reading

- Use the [package guide](README.md), [stable upgrade guide](../../docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels), and [root changelog](../../CHANGELOG.md). A workspace release does not publish Maven coordinates, and native Rust timings are not JNI measurements.

## [0.8.0-alpha.7] - 2026-09-30

### Added

- Added Mermaid 12 Agentflow and Usecase parsing and SVG rendering. Agentflow follows upstream beta syntax; neither family supports ASCII output.

### Changed

- Updated the engine from Mermaid 11.17.2 to 12.0.0, including ELK layout defaults and the new theme/look defaults for supported families. Refresh SVG snapshots; use top-level `layout: dagre`, `theme: default`, and `look: classic` when the previous presentation is needed. The bundled artifact already included ELK and retains its EPL-2.0 notices.
- Safe diagram-local theme variables and fonts are admitted from frontmatter and directives while host security policy remains authoritative.

### Breaking changes

- Added opt-in `auto` ASCII layout for bounded Flowchart and Sequence output, with a Compact retry before overflow handling. ASCII output reports now use schema `3`; update custom decoders for requested/effective layout and Compact-attempt fields.
- Ship the Kotlin classes and native slices from the same AAR. JNI transport API remains `2`; custom ASCII JSON consumers must still adopt the new report schema.

## [0.8.0-alpha.6] - 2026-09-02

This section describes alpha.6, whose matching Android AAR was attached to the GitHub Release on 2026-09-04.

### Added

- Added `MermanOperationControl` with cross-thread cooperative cancellation, optional relative timeouts, cancellation state inspection, and idempotent release. Both `Merman.execute` and `MermanEngine.execute` retain their existing overloads and add controlled dispatch overloads.
- Added structured `MermanCancelledDetails` projection for requested cancellation and deadline expiry. Android JNI transport API 2 owns the opaque control-token registry and controlled native method set.
- Added lossless `MermanExactResourceErrorDetails` for the complete native unsigned 64-bit count range. Existing `resourceDetails` remains available as a signed-`Long` compatibility projection; migrate overflow-sensitive consumers to `exactResourceDetails`.

### Breaking changes

- The default AAR now bundles SVG, both layout engines, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native runtime adapters. The generated helper methods remain stable; unavailable operations return typed missing-capability or unsupported-operation errors. Custom source builds may enable the omitted capabilities.
- Analysis facts now use schema 2 and no longer include the unused Flowchart-only rich graph. Regenerate facts consumers together with the matching native artifact.
- ASCII capability records now expose independent semantic coverage and primary projection fields, and rename `summaryFallback` to `structuredTextFallback`. Structured ASCII resource and diagnostic payloads also follow the expanded six-phase renderer contract; upgrade Kotlin and native slices together.

## [0.8.0-alpha.5] - 2026-08-09

### Breaking changes

- Replaced the C-ABI-forwarding JNI bridge with direct `JNI_OnLoad` + `RegisterNatives` transport API 1. Upgrade the Kotlin classes and `libmerman_android_jni.so` together; alpha.3 `libmerman_ffi.so` JNI slices and ABI 2 checks are incompatible.
- Split the Kotlin source model into `Merman` for discovery and one-shot calls and `MermanEngine(optionsJson, services)` for reusable calls. Removed `MermanReusableEngine` without a compatibility alias.
- Replaced mutable `setTextMeasurer()` configuration with constructor-owned immutable `MermanEngineServices`. Callback-free engines admit concurrent calls; callback-enabled engines return `BUSY` for competing calls and `REENTRANT_CALL` for reentry. `close()` is now nonblocking, retryable, and preserves the handle when an active call prevents closure.
- Replaced the zero-filled `MermanTextMeasureResult` constructor with shape-specific `metrics`, `length`, `horizontalExtents`, and `wrappedWithRawWidth` factories; custom measurers must now provide every field required by the selected shape.
- Replaced parser-backed document facts with their final schema 1 shape. Other versions are rejected before body decoding; remove `fact_source: "text_scan"` handling and consume parser-backed items with explicit unavailable bodies.
- Replaced Options JSON schema 1 with schema 2. Rename `viewport_width` / `viewport_height` to `container_width` / `container_height`, move text/math selectors under `environment`, move semantic theme values under `presentation.theme`, use top-level `site_config` and `svg`, remove the legacy Flowchart ELK selector, and use documented kebab-case values. Request overlays now inherit their constructor resource profile unless one is explicitly supplied.
- Removed `supportedHostThemePresetsJson()` in favor of artifact-aware `presentationCatalogJson()`. Expanded diagram-family capability records require strict custom decoders to upgrade with the matching native slice.

### Added

- Added `execute(operationId, source, optionsJson, uri)` and typed `MermanOperationResult` values for the complete operation catalog. Named SVG, ASCII, analysis, PNG, JPEG, and PDF helpers wrap the same operation path; `*Result` binary helpers retain metadata and effective output plans.
- Added `runtimeCatalogJson()` and generic `metadataJson(id)` discovery with generated operation, output, resource, and text-measurement constants.
- Added `MermanResourceOptionsBuilder` and generated override IDs for `interactive`, `constrained`, `trusted-native`, and `unbounded-for-trusted-input` resource configuration.
- Added `presentationCatalogJson()` for theme preset, presentation profile, aspect, and missing-capability discovery.
- Added immutable `MermanIconPackSet.fromPacks(...)` snapshots and `MermanEngineServices` for constructor-owned icon packs and optional text measurement.

### Changed

- Updated the native engine to the Mermaid 11.16.1 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- JNI text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
- Reusable engine close is idempotent. Engine service destruction occurs only after native admission locks are released, and constructor conflicts fail without invoking callbacks. Icon pack snapshots have no native lifecycle to close.
- The AAR now carries the project license, source-provenance notice, and exact third-party license texts under `META-INF`.
- Android source builds now use the checked-in Gradle Wrapper and pinned JDK 17/NDK toolchain; the build helper can install the required NDK, assemble both published ABIs, and verify the completed AAR in one workflow.

## [0.8.0-alpha.3] - 2026-07-09

### Added

- Documented Android host text-measurement guidance for `MermanReusableEngine` callbacks.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## [0.8.0-alpha.2] - 2026-06-23

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.2` release.

## [0.8.0-alpha.1] - 2026-06-10

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.1` release.

## [0.7.0] - 2026-06-09

### Changed

- Updated package metadata for the merman workspace `0.7.0` release.
- Added host theme preset discovery through the Android JNI wrapper.

## [0.7.0-alpha.2] - 2026-06-08

### Changed

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## [0.7.0-alpha.1] - 2026-06-05

### Added

- Initial experimental Android JNI package for the merman C ABI.

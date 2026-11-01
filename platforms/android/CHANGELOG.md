# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [Unreleased]

### Breaking changes

- Advance Options JSON from schema `2` to `3`: move visual styling from `presentation.theme` to the closed `theme` preset-or-spec group, and use `raster.matte` and `pdf.page_paint` for export backgrounds. Removed presentation-profile inputs are rejected. Regenerate Kotlin helpers and replace presentation discovery with `themeCatalogJson()`.
- For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes`; SVG-element and native SVG-backend overrides are rejected for this operation. The output remains schema `1`.
- Constrained resource policy rejects formulas even in the new math-capable product. Handle the rejection or choose an appropriate interactive/trusted policy for permitted workloads with host execution and memory limits.

### Added

- Include math in the ordinary SVG product. Retain the full selected-version Maven repository, qualify it from an external coordinate consumer with 16 KB native/APK checks, and prepare a signed standard Central upload bundle under an explicitly verified namespace. Preparing the bundle does not publish to Central.

- Added shared theme materialization, support-query and preset-export operations with structured authoring diagnostics and theme-specific resource budgets. Both one-shot and reusable consumers retain the same admission contract; catalog availability does not grant Portable support.
- Generated resource options now include `maxPreparedTextRetainedBytes`, whose defaults are 24 MiB for `interactive`, 12 MiB for `constrained`, 128 MiB for `trusted-native`, and unlimited for trusted unbounded input.

## [0.9.3] - 2026-10-30

These changes are available in the `v0.9.3` source. The published AAR release asset remains at `0.9.2`.

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

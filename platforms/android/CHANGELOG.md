# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

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

### Breaking changes

- Advance Options JSON from schema `2` to `3`: move visual styling from `presentation.theme` to the closed `theme` preset-or-spec group, and use `raster.matte` and `pdf.page_paint` for export backgrounds. Removed presentation-profile inputs are rejected. Regenerate Kotlin helpers and replace presentation discovery with `themeCatalogJson()`.

### Added

- Added shared theme materialization, support-query and preset-export operations with structured authoring diagnostics and theme-specific resource budgets. Both one-shot and reusable consumers retain the same admission contract; catalog availability does not grant Portable support.
- Generated resource options now include `maxPreparedTextRetainedBytes`, whose defaults are 24 MiB for `interactive`, 12 MiB for `constrained`, 128 MiB for `trusted-native`, and unlimited for trusted unbounded input.

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
- ASCII capability records now expose independent semantic coverage and primary projection fields,
  and rename `summaryFallback` to `structuredTextFallback`. Structured ASCII resource and
  diagnostic payloads also follow the expanded six-phase renderer contract; upgrade Kotlin and
  native slices together.

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

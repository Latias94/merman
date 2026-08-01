# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.8.0-alpha.4] - 2026-07-26

### Breaking changes

- Replaced the C-ABI-forwarding JNI bridge with a direct `JNI_OnLoad` + `RegisterNatives`
  transport over `merman-bindings-core` in a dedicated internal crate. Kotlin classes and the new
  `libmerman_android_jni.so` from this release must be upgraded together; older
  `libmerman_ffi.so` JNI slices are incompatible.
- Replaced byte-only generic execution with `execute(operationId, source, optionsJson, uri)`, which returns `MermanOperationResult(operationId, mediaType, data, metadataJson)` for both one-shot and reusable engines. SVG, ASCII, JSON, PNG, JPEG, and PDF convenience methods delegate to the same operation path and unpack `data`.
- Replaced mutable `setTextMeasurer` with an immutable `textMeasurer` constructor argument. Callback-free reusable engines permit concurrent calls; callback-enabled engines return `BUSY` for a competing call and `REENTRANT_CALL` for callback reentry.
- Replaced blocking/destructive `nativeFree` close with nonblocking `nativeTryClose`. A failed `close()` preserves the Kotlin handle and can be retried after the active call completes.
- Replaced `runtimeContractJson()` with `runtimeCatalogJson()`. The new direct catalog is a flat
  schema-1 document containing package identity, sorted capability/output/operation IDs, registry
  facts, resource descriptors, and text-measurement providers; it validates Android transport API
  version `1` and intentionally has no C ABI version field.
- Replaced the zero-filled `MermanTextMeasureResult` constructor with shape-specific `metrics`, `length`, `horizontalExtents`, and `wrappedWithRawWidth` factories; custom measurers must now provide every field required by the selected shape.
- Replaced parser-backed document facts with their final schema 1 shape. Other versions are rejected before body decoding; remove `fact_source: "text_scan"` handling and consume parser-backed items with explicit unavailable bodies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the legacy Flowchart ELK backend selector; update serialized `optionsJson` before upgrading.
- Moved binding JSON environment selectors to `environment.text_measurement` and `environment.math_renderer`, and theme variables to `host_theme.theme_variables`; remove legacy `layout.text_measurer`, `layout.math_renderer`, and `host_theme.themeVariables` keys before upgrading because they are now rejected.
- Removed underscore and shorthand binding enum aliases. Use the documented kebab-case values such as `resvg-safe`, `strip-existing-important`, `trusted-native`, and `unbounded-for-trusted-input`, plus generated host-theme preset names.
- Expanded the diagram-family capability JSON. Upgrade custom strict Kotlin/JSON decoders with the native library; the canonical record now includes logical/render-model identities, parser/render flags, authoring header, and configuration namespace.
- Replaced the incompatible prerelease options grammar with Options JSON schema `2`. `MermanResourceOptionsBuilder` now leaves the profile unset by default so request overlays inherit their constructor ceiling, and only generated `MermanResourceOverrideId` values can be supplied as overrides.

### Added

- Added generated Kotlin text-measurement operation/result-kind constants.
- Added the generated `MermanResourceOptionsBuilder` and runtime resource catalog so Android
  callers can select `interactive`, `constrained`, `trusted-native`, or
  `unbounded-for-trusted-input` without duplicating limit tables.

### Changed

- Updated the native engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- JNI text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
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

# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.8.0-alpha.6] - Unreleased

This section describes the prepared alpha.6 source candidate. The Android artifact channel remains independently published and requires the matching Kotlin sources and native library before adoption.

### Added

- Added `MermanOperationControl` with cross-thread cooperative cancellation, optional relative timeouts, cancellation state inspection, and idempotent release. Both `Merman.execute` and `MermanEngine.execute` retain their existing overloads and add controlled dispatch overloads.
- Added structured `MermanCancelledDetails` projection for requested cancellation and deadline expiry. Android JNI transport API 2 owns the opaque control-token registry and controlled native method set.
- Added lossless `MermanExactResourceErrorDetails` for the complete native unsigned 64-bit count range. Existing `resourceDetails` remains available as a signed-`Long` compatibility projection; migrate overflow-sensitive consumers to `exactResourceDetails`.

### Breaking changes

- The default AAR now bundles SVG, both layout engines, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native runtime adapters. The generated helper methods remain stable; unavailable operations return typed missing-capability or unsupported-operation errors. Custom source builds may enable the omitted capabilities.
- Analysis facts now use schema 2 and remove the unused Flowchart-only rich graph; regenerate facts consumers together with the matching native artifact.
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

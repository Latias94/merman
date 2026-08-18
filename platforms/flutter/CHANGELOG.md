# Changelog

## [Unreleased]

### Added

- Added `MermanOperationControl` for reusable cooperative deadlines and cancellation, including
  structured `MermanCancelledException` details. Controls are isolate-local; same-isolate timers
  cannot interrupt a synchronous execution call.
- Added `MermanExactResourceErrorDetails` so unsigned 64-bit resource counts remain available as
  canonical decimal strings. The existing signed-`int` projection remains available when both
  counts fit its compatibility range.
- Added `MermanDiagnosticErrorDetails` and `MermanDiagnosticSpan` so parser and ASCII renderer
  failures preserve native diagnostic codes, byte spans, fields, and diagram types without
  parsing human-facing messages.

### Breaking changes

- The next workspace release will publish analysis facts schema 2 and remove the unused Flowchart-only rich graph; regenerate facts consumers together with the matching native artifact.
- ASCII capability records now expose independent semantic coverage and primary projection fields,
  and rename `summaryFallback` to `structuredTextFallback`. Structured ASCII resource and
  diagnostic payloads follow the expanded six-phase renderer contract; upgrade Dart and bundled
  native artifacts together.

## 0.8.0-alpha.5 - 2026-08-12

This is the first pub.dev release of `0.8.0-alpha.5`. It is built from the reviewed Flutter release commit rather than the earlier workspace tag.

### Breaking changes

- The native libraries bundled on pub.dev now provide SVG, both layout engines, ASCII, analysis, validation, and document analysis. Math, PNG, JPEG, PDF, and native runtime adapters require a current-contract custom library loaded through `Merman.openPath(...)` or `Merman.fromDynamicLibrary(...)`; bundled calls fail with typed missing-capability or unsupported-operation errors.
- Replaced the legacy Flutter plugin integration with Dart `package_ffi` and Native Assets. The package now requires Dart 3.10 or newer; Flutter consumers require Flutter 3.38 or newer, Android API 24 or newer, iOS 13 or newer, and macOS 11 or newer. The public rendering API remains unchanged, while the platform plugin registrars and `openMermanLibrary()` helper are removed.
- Replaced the prerelease ABI 2 wrapper with generated ABI 3 table discovery. Upgrade the Dart package and all bundled native artifacts together; direct `merman_*` symbol lookup, manually maintained raw FFI records, and partial-table compatibility are no longer supported.
- Split the public SDK into a stateless `Merman` discovery/one-shot facade and a directly constructed, explicitly closeable `MermanEngine`. Replace `MermanReusableEngine`, `Merman.reusableEngine(...)`, engine-owning `Merman` instances, `dispose()`, and callback-specialized facade constructors.
- Moved host text measurement and icon packs into immutable constructor-owned `MermanEngineServices`. The post-construction callback API is removed; callback-enabled engines report typed `BUSY` or `REENTRANT` failures, and engine close is retryable and idempotent.
- Replaced format-specific option envelopes with generic `optionsJson` and Options JSON schema `2`. Rename viewport fields to `container_width` / `container_height`, move text/math selectors under `environment`, move semantic theme values under `presentation.theme`, use top-level `site_config` and `svg`, remove the legacy Flowchart ELK selector, and use documented kebab-case values.
- Replaced parser-backed document facts with their final schema 1 shape, raw operation metadata maps with typed `MermanOperationMetadata` / `MermanOutputPlan` values, and closed operation/resource enums with generated open value objects. Iterate `.knownValues`, preserve unknown output plans through `rawJson`, and use `presentationCatalog()` instead of `supportedHostThemePresets()`.

### Added

- Added validated `MermanRuntimeCatalog` discovery plus generated operation, payload, resource, metadata, and constructor-service contracts for the exact loaded artifact.
- Added generic execution and named helpers for SVG, PNG, JPEG, PDF, ASCII, semantic/layout/analysis JSON, document analysis, and validation, including typed raster/PDF output plans and result-returning binary helpers.
- Added immutable `MermanIconPack` / `MermanIconPackSet`, generated resource builders for the standard runtime profiles, and typed cached diagram, parser/render, lint, Mermaid-theme, and presentation metadata APIs.

### Changed

- Flutter native builds now consume the shared size-oriented default native artifact recipe, bundle precompiled libraries through one Native Assets hook, and enforce the pub.dev compressed-upload budget during release preflight.
- Added Android `armeabi-v7a` to the packaged native matrix alongside `arm64-v8a` and `x86_64`.
- Updated the bundled engine to the Mermaid 11.16.1 compatibility baseline and the shared 35-family parser, layout, SVG, theme, editor, and resource-limit contracts.
- Flutter native builds consume the exact artifact recipe; Apple slices compile-check the complete public C headers, and the pub package carries the project license, source provenance, and exact third-party license texts.

### Fixed

- Native Assets removes the legacy macOS dylib relocation, Linux Windows-wrapper linkage, and SwiftPM symlink failure surfaces reported in #55, #56, and #57. Apple install names are normalized before signing, and Flutter performs final framework assembly and signing.

## 0.8.0-alpha.3 - 2026-07-09

- Documented Flutter/Dart host text-measurement guidance for `MermanReusableEngine` callbacks.
- Added pub.dev metadata links and README compatibility notes for C ABI release discovery.
- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## 0.8.0-alpha.2 - 2026-06-23

- Updated package metadata for the merman workspace `0.8.0-alpha.2` release.

## 0.8.0-alpha.1 - 2026-06-10

- Updated package metadata for the merman workspace `0.8.0-alpha.1` release.

## 0.7.0 - 2026-06-09

- Updated package metadata for the merman workspace `0.7.0` release.
- Added host theme preset and supported theme discovery through the bundled native bindings.

## 0.7.0-alpha.2 - 2026-06-08

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## 0.7.0-alpha.1 - 2026-06-05

- Initial experimental Flutter/Dart FFI package for the merman C ABI, including bundled native
  artifacts for Android, iOS, macOS, Windows, and Linux.

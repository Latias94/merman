# Changelog

All notable changes to the Apple Swift package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [Unreleased]

### Breaking changes

- Advanced the direct UniFFI binding API to `6` because `MermanAsciiCapability` gained
  layout/width/encoding/fallback admission arrays and `MermanAsciiOutputPlan` gained schema-2
  encoding. API 6 replaces `bindingApiVersionV5()` with `bindingApiVersionV6()` so stale generated
  Swift fails before decoding either changed record. Regenerate Swift and replace the XCFramework
  together.
- Renamed generic dispatch records to `MermanOperationRequestV4` and added optional `MermanOperationControl` values for cooperative cancellation and relative deadlines. Cancellation is a distinct generated error detail with its observed reason and phase; it is not a resource-limit failure.
- Advanced the direct UniFFI binding API to `5` because `MermanAsciiCapability` and
  `MermanError.Binding` changed wire layouts that UniFFI method checksums do not cover. API 5
  replaces `transportApiVersion()` with `bindingApiVersionV5()` and removes the API 4 probe symbol,
  so stale generated Swift rejects the new library before decoding either record. Regenerate Swift
  and replace the XCFramework together.
- The default XCFramework now bundles SVG, both layout engines, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native runtime adapters. Generated helpers remain available for custom artifacts; the bundled library reports typed capability absence instead of carrying every optional backend.
- Analysis facts now use schema `2` and no longer include the Flowchart-only rich graph. Regenerate facts consumers for schema `2`; diagnostics remain on schema `1`.
- ASCII capability records now expose independent semantic coverage and primary projection fields,
  and rename `summaryFallback` to `structuredTextFallback`. Structured ASCII resource and
  diagnostic payloads follow the expanded six-phase renderer contract.

### Changed

- Kept Apple static-library slices on the `native-sdk` profile after a same-source link comparison showed the size-oriented dynamic-library profile produced a larger final Swift executable.

## [0.8.0-alpha.5] - 2026-08-09

### Breaking changes

- Replaced the hand-written Swift C binding with direct generated UniFFI bindings. `Merman` now owns discovery and one-shot calls, while reusable work uses the single throwing `MermanEngine(optionsJson:services:)` constructor. The obsolete `MermanReusableEngine` name and facade factories are removed.
- Removed all public C ABI structs, raw callback pointers, manual engine close methods, struct-size checks, and hand-maintained Swift capability/resource projections. Swift hosts now use generated UniFFI records, objects, and callback protocols only.
- Replaced native ABI version checks with UniFFI binding API `3` and introduced runtime-contract schema `1`. Structured resource failures include a stable `cause` field; the generated binding rejects a mismatched native library through its contract and API checksum checks.
- Replaced raw C text-measurement callbacks and mutable callback installation with generated `MermanTextMeasurer` services supplied at reusable-engine construction. Return `nil` for an unhandled operation; callback-enabled engines report typed `.busy` or `.reentrantCall` errors without waiting.
- Replaced the incompatible prerelease options grammar with Options JSON schema `2`. The generated `resourceOptionsJson(profile:overrides:)` API now accepts a `nil` profile for request overlays that inherit their constructor ceiling, and its override records use `MermanResourceOverrideId`.
- Removed the prerelease `supportedHostThemePresets()` method. Decode `presentationCatalogJson()` for open-ended, artifact-aware theme preset and presentation profile discovery.

### Added

- Added checked-in `Merman.swift`, `MermanFFI.h`, and `MermanFFI.modulemap` generated from the exact `merman-uniffi` static library included in the XCFramework.
- Added atomic `runtimeCatalogJson()` discovery for package identity, capabilities, operations, outputs, resources, and text-measurement providers.
- Added generic operation requests/results plus named SVG, PNG, JPEG, and PDF helpers. Results expose typed `MermanOperationMetadata` and open `MermanOutputPlan` records while retaining `rawJson` for future plan kinds.
- Added immutable `MermanIconPack`, transactional `MermanIconRegistry.fromPacks(...)`, and persistent `MermanEngineServices` for constructor-owned icon registries and optional text measurement. Reusable engines expose retryable, idempotent `close()`.
- Added generated `resourceOptionsJson(profile:overrides:)` so Swift callers can select `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input` without duplicating limit tables.
- Added generated `presentationCatalogJson()` without changing the UniFFI API 3 version.

### Changed

- Updated the bundled engine to the Mermaid 11.16.1 compatibility baseline and the shared 35-family parser, layout, SVG, theme, and editor contracts.
- Added optional `optionsJson` to reusable convenience methods. Pass `nil` to inherit the engine baseline or provide a request-local deep merge; request options cannot change constructor-owned runtime policy.
- Generated lint and text-measurement APIs remain present across feature profiles. Feature-slim artifacts report typed `analysis` or `svg` missing-capability errors instead of returning empty catalogs or omitting callback types.
- The XCFramework now packages `libmerman_uniffi.a` with the matching generated UniFFI header and module map for every Apple slice.
- XCFramework archives now carry the project license, source-provenance notice, and exact third-party license texts beside the binary bundle.

## [0.8.0-alpha.3] - 2026-07-09

### Added

- Documented Apple host text-measurement guidance for `MermanReusableEngine` callbacks.
- Added Swift `MermanTextMeasureCallback`, `MermanTextMeasureRequest`, and
  `MermanTextMeasureResult` aliases for host text-measurement callbacks.

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
- Added host theme preset discovery through the Swift wrapper.

## [0.7.0-alpha.2] - 2026-06-08

### Changed

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## [0.7.0-alpha.1] - 2026-06-05

### Added

- Initial experimental SwiftPM package for the merman C ABI on iOS and macOS.

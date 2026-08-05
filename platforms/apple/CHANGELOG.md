# Changelog

All notable changes to the Apple Swift package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.8.0-alpha.4] - Unreleased

### Breaking changes

- Replaced the hand-written Swift C binding with direct generated UniFFI bindings. `MermanEngine` is now constructed as `MermanEngine()`; use generated camel-case method labels such as `renderSvg(source:optionsJson:)`, `execute(request:)`, and `runtimeCatalogJson()`. Generic options now belong to `MermanOperationRequest.optionsJson`.
- Replaced split runtime-contract and capability-vocabulary discovery with one atomic `runtimeCatalogJson()` response. The generated API no longer exposes either legacy endpoint.
- Removed all public C ABI structs, raw callback pointers, manual engine close methods, struct-size checks, and hand-maintained Swift capability/resource projections. Swift hosts now use generated UniFFI records, objects, and callback protocols only.
- Replaced native ABI version checks with UniFFI binding API `3` and introduced runtime-contract schema `1`. Structured resource failures include a stable `cause` field; the generated binding rejects a mismatched native library through its contract and API checksum checks.
- Replaced the C callback text-measurement API with generated `MermanTextMeasurer`. Return `nil` for an unhandled operation rather than populating a raw result buffer.
- Made `MermanTextMeasurer` immutable after reusable-engine construction and removed generated callback mutation methods. Callback-free engines admit concurrent operations; callback engines report typed `.busy` or `.reentrantCall` errors without waiting.
- Changed `lintRuleCatalog()` and `configurableLintRuleCatalog()` to throwing generated methods so feature-slim artifacts report a typed `analysis` missing-capability error instead of an empty catalog.
- Added `optionsJson` to reusable convenience methods. Pass `nil` to inherit the engine baseline, or pass request-local options to deeply merge them for one operation; request options cannot change the constructor-owned runtime policy.
- Replaced the incompatible prerelease options grammar with Options JSON schema `2`. The generated `resourceOptionsJson(profile:overrides:)` API now accepts a `nil` profile for request overlays that inherit their constructor ceiling, and its override records use `MermanResourceOverrideId`.
- Removed the prerelease `supportedHostThemePresets()` method. Decode `presentationCatalogJson()` for open-ended, artifact-aware theme preset and presentation profile discovery.

### Added

- Added checked-in `Merman.swift`, `MermanFFI.h`, and `MermanFFI.modulemap` generation from the
  exact `merman-uniffi` static library included in the XCFramework.
- Added generic operation requests/results and direct SVG, PNG, JPEG, and PDF smoke coverage.
- Added generated `resourceOptionsJson(profile:overrides:)` so Swift callers can select
  `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input` without
  duplicating limit tables.
- Added generated `presentationCatalogJson()` without changing the UniFFI API 3 version.

### Changed

- The XCFramework now packages `libmerman_uniffi.a` with the matching generated UniFFI header and
  module map for every Apple slice.
- Generated text-measurement protocols and reusable-engine entrypoints remain present across
  feature profiles; artifacts without SVG report a typed `svg` missing-capability error when used.
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

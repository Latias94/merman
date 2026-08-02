# Changelog

## 0.8.0-alpha.4 - Unreleased

### Breaking changes

- Rebuilt the Dart FFI transport against the frozen ABI 3 minimum prefix. The pre-freeze layout digest and `engine_free` table slot are no longer accepted; discovery now uses the minimum-prefix digest and `engine_try_close`.
- Replaced the prerelease ABI 2 wrapper with ABI 3 table discovery. Direct `merman_*` symbol lookup, manually maintained raw Dart FFI records, and ABI 2 compatibility paths are removed. Upgrade the Dart package and its bundled native artifacts together.
- Moved host text measurement to reusable-engine construction: pass `textMeasurer:` to `Merman.reusableEngine(...)`. The former post-construction callback installation API is removed.
- Replaced format-specific option envelopes with generic `optionsJson` on `execute` and every convenience method. Request options deeply override the reusable engine baseline for one call; `runtime_policy` remains constructor-owned.
- Replaced parser-backed document facts with their final schema 1 shape. Other versions are rejected before body decoding; remove `fact_source: "text_scan"` handling and consume parser-backed items with explicit unavailable bodies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the legacy Flowchart ELK backend selector; update serialized `optionsJson` before upgrading.
- Moved binding JSON environment selectors to `environment.text_measurement` and `environment.math_renderer`, semantic host colors to `presentation.theme`, raw Mermaid overrides to top-level `site_config`, and output policy to `svg`. The prerelease `host_theme` group and the old `layout.text_measurer` / `layout.math_renderer` fields are rejected.
- Removed underscore and shorthand binding enum aliases plus `supportedHostThemePresets()`. Use documented kebab-case values and the open-ended typed `presentationCatalog()` result instead of a closed preset list.
- Replaced the incompatible prerelease options grammar with Options JSON schema `2`. Generated resource builders now leave the profile unset by default so request overlays inherit their constructor ceiling, and only `MermanResourceOverrideId` values can be written as overrides.

### Added

- Added opaque native result allocation-token ownership, typed BUSY and REENTRANT exceptions, optional immutable text measurement on `Merman` construction, and ABI-compatible `openPath` loading.
- Added ffigen-generated ABI 3 declarations, checked against the native table's version, digest,
  and function pointers before use.
- Added a strict flat `MermanRuntimeCatalog` that validates the loaded artifact's capability,
  output, adapter, registry, resource, and text-measurement facts before enabling calls.
- Added generic output execution and copied output bytes alongside Dart conveniences for
  SVG, PNG, JPEG, PDF, ASCII, semantic/layout/analysis JSON, document analysis, and validation.
- Added generated resource options so Flutter callers can choose `interactive`, `constrained`,
  `trusted-native`, or `unbounded-for-trusted-input` without copying limit tables.
- Restored typed and cached diagram, ASCII, parser/render, lint-rule, Mermaid-theme, and presentation metadata APIs through the appended ABI 3 `metadata_collect` slot. Exact-version package loading requires the slot, while `openPath` retains frozen five-slot prefix compatibility.

### Changed

- Updated the bundled engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Flutter native build helpers now consume the exact artifact feature recipe.
- Flutter Apple XCFramework slices now bundle and compile-check the complete public C header set.
- The pub package now carries the project license, source-provenance notice, and exact third-party license texts.

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

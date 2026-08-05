# Changelog

All notable changes to the Python package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.8.0a4] - Unreleased

Corresponds to merman workspace release `0.8.0-alpha.4`.

### Breaking changes

- Replaced the prerelease UniFFI ABI 2 surface with direct UniFFI binding API 3. Structured resource failures include a stable `cause` field. The API remains independent from the native C ABI and text-measurement protocol; regenerate and deploy the generated Python package with its exact native library rather than mixing releases.
- Introduced runtime-contract schema 1 with stable capability, operation, output, system-adapter, and optional text-measurement provider IDs. Python validates these against the engine-owned runtime catalog and rejects unknown, duplicate, unsorted, or incoherent IDs.
- Replaced the prerelease text-measurement callback records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`line_count`; upgrade the Python wheel and bundled native library together and update custom measurers for operations `0..18`.
- Made `MermanTextMeasurer` immutable after reusable-engine construction and removed `set_text_measurer()` / `clear_text_measurer()`. Callback-free engines admit concurrent operations; callback engines raise typed `BUSY` or `REENTRANT_CALL` errors without waiting.
- Replaced parser-backed document facts with their final schema 1 shape. Other versions are rejected before body decoding; remove `fact_source: "text_scan"` handling and consume parser-backed items with explicit unavailable bodies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the legacy Flowchart ELK backend selector; update any serialized `options_json` before upgrading.
- Moved binding JSON environment selectors to `environment.text_measurement` and `environment.math_renderer`, semantic host colors to `presentation.theme`, raw Mermaid overrides to top-level `site_config`, and output policy to `svg`. The prerelease `host_theme` group and the old `layout.text_measurer` / `layout.math_renderer` fields are rejected.
- Removed underscore and shorthand binding enum aliases plus `supported_host_theme_presets()`. Use documented kebab-case values and decode `presentation_catalog_json()` for open-ended theme/profile discovery.
- Removed generated `ABI_VERSION` and `require_abi_version()` helpers. Use `MermanEngine.binding_api_version()` for the UniFFI transport version, `get_runtime_catalog()` for a validated runtime catalog, and the separate text-measurement protocol helper for callback compatibility.
- Removed split `runtime_contract_json()` and `runtime_capability_vocabulary_json()` discovery. Use the one atomic `runtime_catalog_json()` endpoint and `get_runtime_catalog()` decoder.
- Moved generic operation options into `MermanOperationRequest.options_json`; call `engine.execute(request)` without a parallel options argument. Reusable operation options now deeply merge over the engine baseline but cannot change its constructor-owned runtime policy.
- Added `options_json` to reusable convenience methods. Pass `None` to inherit the engine baseline or provide a request-local override for that operation.
- Replaced the incompatible prerelease options grammar with Options JSON schema `2`. New resource helpers emit version `2`, omit `resources.profile` when a request should inherit its constructor ceiling, and accept only generated `ResourceOverrideId` values for overridable limits.

### Added

- Added `MermanOperationRequest`, `MermanOperationResult`, and `MermanEngine.execute()` as the one descriptor-owned operation path. Named methods are wrappers over it.
- Added real `render_png()`, `render_jpeg()`, and `render_pdf()` byte APIs when the matching artifact output capability is enabled.
- Added the generated `ResourceOptionsBuilder` and schema `2` resource contract so Python callers can select `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input` without duplicating limit tables.
- Added `presentation_catalog_json()` for artifact-aware theme preset, presentation profile, aspect, and missing-capability discovery.

### Changed

- Updated the bundled engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Generated lint and host text-measurement APIs now keep the same shape across feature profiles. Feature-slim artifacts raise typed `analysis` or `svg` missing-capability errors instead of returning an empty lint catalog or omitting callback types from the package.
- Host text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
- Wheels now carry the project license, source-provenance notice, and exact third-party license texts in `.dist-info/licenses`.

## [0.8.0a3] - 2026-07-09

Corresponds to merman workspace release `0.8.0-alpha.3`.

### Added

- Added PyPI changelog metadata and README compatibility notes for UniFFI ABI and release discovery.
- Added UniFFI ABI 2 with reusable engines, diagram-family capability discovery, and host text-measurement callbacks that can be installed or cleared on reusable engines.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## [0.8.0a2] - 2026-06-23

Corresponds to merman workspace release `0.8.0-alpha.2`.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.2` release.

## [0.8.0a1] - 2026-06-10

Corresponds to merman workspace release `0.8.0-alpha.1`.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.1` release.

## [0.7.0] - 2026-06-09

Corresponds to merman workspace release `0.7.0`.

### Changed

- Updated package metadata for the merman workspace `0.7.0` release.
- Added host theme preset discovery through the UniFFI Python package.

## [0.7.0a2] - 2026-06-08

Corresponds to merman workspace release `0.7.0-alpha.2`.

### Changed

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## [0.7.0a1] - 2026-06-05

Corresponds to merman workspace release `0.7.0-alpha.1`.

### Added

- Initial experimental Python package for the merman UniFFI bindings.

# Changelog

All notable changes to the Python package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.8.0] - Unreleased

### Fixed

- Corrected SVG relationship geometry and layering for C4 and Class/ELK, restored Usecase Neo styling and explicit ER table styling, and prevented malformed Unicode color values from causing a panic.

### Upgrade

- When upgrading from `0.7.0`, use the [stable upgrade guide](../../../docs/release/V070_TO_V080_UPGRADE_GUIDE.md) and [comparison report](../../../docs/release/V070_TO_V080_RELEASE_REPORT.md) for the accumulated runtime, payload, presentation, and package changes. Upgrade the package facade and its matching artifacts together; this entry prepares the stable candidate and does not announce publication.

### Breaking changes

- Regenerate the direct UniFFI bindings and native library together for the typed-theme authoring and diagnostic changes. Published alpha.7 already exposed API `7` through `binding_api_version_v7()`; that historical number does not establish compatibility with this development interface. Schema-3 ASCII plans retain requested/effective layouts and Compact-attempt information.

- Replace `presentation_catalog_json()` with `theme_catalog_json()` for versioned theme catalog discovery and the shared theme authoring operations; errors retain their structured `theme_authoring` diagnostic envelope.

- Advance Options JSON from schema `2` to `3`: use the closed top-level `theme` preset-or-spec group, `raster.matte`, and `pdf.page_paint`. Removed `presentation` paths and host-owned CSS/security fields are rejected; update saved options alongside the generated bindings.

## [0.8.0a7] - 2026-09-30

### Added

- Added Mermaid 12 Agentflow and Usecase parsing and SVG rendering. Agentflow follows upstream beta syntax; neither family supports ASCII output.

### Changed

- Updated the engine from Mermaid 11.17.2 to 12.0.0, including ELK layout defaults and the new theme/look defaults for supported families. Refresh SVG snapshots; use top-level `layout: dagre`, `theme: default`, and `look: classic` when the previous presentation is needed. The bundled artifact already included ELK and retains its EPL-2.0 notices.
- Safe diagram-local theme variables and fonts are admitted from frontmatter and directives while host security policy remains authoritative.

### Breaking changes

- Added opt-in `auto` ASCII layout for bounded Flowchart and Sequence output, with a Compact retry before overflow handling. ASCII output reports now use schema `3`; update custom decoders for requested/effective layout and Compact-attempt fields.
- Direct UniFFI binding API advances to `7`. Regenerate Python wrappers and the native library together; use `binding_api_version_v7()` to reject stale record layouts. The generic request remains `MermanOperationRequestV4`.

## [0.8.0a6] - 2026-09-02

This section describes alpha.6, published to PyPI on 2026-09-04 with matching macOS arm64, manylinux x86_64, and Windows x86_64 wheels.

### Breaking changes

- Advanced the direct UniFFI binding API to `6` because `MermanAsciiCapability` gained layout/width/encoding/fallback admission arrays and `MermanAsciiOutputPlan` gained schema-2 encoding. API 6 replaces `binding_api_version_v5()` with `binding_api_version_v6()` so stale generated packages fail before decoding either changed record. Regenerate and deploy the Python package and native library together.
- Renamed generic dispatch records to `MermanOperationRequestV4` and added optional `MermanOperationControl` values for cooperative cancellation and relative deadlines. `MermanError.Binding.cancellation` reports the observed reason and phase independently from resource-limit details.
- Default wheels now bundle SVG, both layout engines, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native runtime adapters. The generated API remains stable; unavailable operations return typed missing-capability or unsupported-operation errors, and custom source builds may enable the omitted capabilities.
- Analysis facts now use schema 2 and remove the unused Flowchart-only rich graph; regenerate facts consumers together with the matching native artifact.
- ASCII capability records now expose `semantic_coverage` and `primary_projection`, and rename `summary_fallback` to `structured_text_fallback`. `MermanOutputPlan.ascii` exposes the schema-2 terminal projection and viewport outcome, while structured ASCII diagnostics include the requested and actual width context.
- Wheel builds derive archive timestamps from the accepted source commit, and publication completes only after PyPI exposes the exact expected wheel SHA-256 values.

## [0.8.0a5] - 2026-08-09

Corresponds to merman workspace release `0.8.0-alpha.5`.

### Breaking changes

- Replaced the prerelease UniFFI ABI 2 surface with direct UniFFI binding API 3. Structured resource failures include a stable `cause` field. The API remains independent from the native C ABI and text-measurement protocol; regenerate and deploy the generated Python package with its exact native library rather than mixing releases.
- Renamed the discovery/one-shot facade to `Merman` and the reusable type to `MermanEngine`. Reusable engines now have one direct `MermanEngine(options_json, services)` constructor; the obsolete `MermanReusableEngine` name, facade factories, and callback-specialized constructors are removed.
- Replaced the prerelease text-measurement callback records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`line_count`; upgrade the Python wheel and bundled native library together and update custom measurers for operations `0..18`.
- Made `MermanTextMeasurer` immutable after reusable-engine construction and removed `set_text_measurer()` / `clear_text_measurer()`. Callback-free engines admit concurrent operations; callback engines raise typed `BUSY` or `REENTRANT_CALL` errors without waiting.
- Replaced parser-backed document facts with their final schema 1 shape. Other versions are rejected before body decoding; remove `fact_source: "text_scan"` handling and consume parser-backed items with explicit unavailable bodies.
- Replaced Options JSON schema 1 with schema 2. Rename `viewport_width` / `viewport_height` to `container_width` / `container_height`, move text/math selectors under `environment`, move semantic theme values under `presentation.theme`, use top-level `site_config` and `svg`, remove the legacy Flowchart ELK selector, and use documented kebab-case values. Request overlays inherit their constructor resource profile unless one is explicitly supplied.
- Removed `supported_host_theme_presets()` in favor of artifact-aware `presentation_catalog_json()`.
- Removed generated `ABI_VERSION` and `require_abi_version()` helpers. Use `Merman.binding_api_version()` for the UniFFI transport version, `get_runtime_catalog()` for a validated runtime catalog, and the separate text-measurement protocol helper for callback compatibility.

### Added

- Added runtime-contract schema 1 with atomic `runtime_catalog_json()` / `get_runtime_catalog()` discovery for package identity, capabilities, operations, outputs, system adapters, resources, and optional text-measurement providers.
- Added `MermanOperationRequest`, `MermanOperationResult`, and `MermanEngine.execute()` as the descriptor-owned operation path. Put request-local options in `MermanOperationRequest.options_json`; named methods are wrappers over the same operation catalog.
- Added real `render_png()`, `render_jpeg()`, and `render_pdf()` byte APIs when the matching artifact output capability is enabled.
- Added typed `MermanOperationMetadata` and open `MermanOutputPlan` records for raster/PDF plans while retaining `raw_json` for future plan kinds.
- Added immutable `MermanIconPack`, transactional `MermanIconRegistry.from_packs()`, and persistent `MermanEngineServices` for constructor-owned icon registries and optional text measurement. Reusable engines expose retryable, idempotent `close()`.
- Added the generated `ResourceOptionsBuilder` and schema `2` resource contract so Python callers can select `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input` without duplicating limit tables.
- Added `presentation_catalog_json()` for artifact-aware theme preset, presentation profile, aspect, and missing-capability discovery.

### Changed

- Updated the bundled engine to the Mermaid 11.16.1 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Added optional `options_json` to reusable convenience methods. Pass `None` to inherit the engine baseline or provide a request-local deep merge; request options cannot change constructor-owned runtime policy.
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

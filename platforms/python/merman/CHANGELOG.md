# Changelog

All notable changes to the Python package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [Unreleased]

### Added

- Include math in the ordinary SVG wheel product without adding image encoders or ambient font services. Wheel candidates cover macOS ARM64/x86-64, GNU Linux ARM64/x86-64 and Windows x86-64; Linux candidates are built, repaired and installed at manylinux 2.28. Normal wheel installation does not require Rust.

### Breaking changes

- Regenerate the direct UniFFI bindings and native library together for typed-theme authoring and diagnostics. UniFFI API `7` is unchanged; that number does not make wrappers from different releases interchangeable.
- Replace `presentation_catalog_json()` with `theme_catalog_json()` for versioned theme catalog discovery and the shared theme authoring operations; errors retain their structured `theme_authoring` diagnostic envelope.
- Advance Options JSON from schema `2` to `3`: use the closed top-level `theme` preset-or-spec group, `raster.matte`, and `pdf.page_paint`. Removed `presentation` paths and host-owned CSS/security fields are rejected; update saved options alongside the generated bindings.
- For `edge-geometry-json`, replace operation-local `max_svg_bytes` with `max_geometry_json_bytes`; SVG-element and native SVG-backend overrides are rejected for this operation. The output remains schema `1`.
- Constrained resource policy rejects formulas even in the new math-capable wheel. Handle the rejection or choose an appropriate interactive/trusted policy for permitted workloads with host execution and memory limits.

## [0.9.3] - 2026-10-30

These changes are available in the `v0.9.3` source. The published PyPI package remains at `0.9.2`.

- Fix clipped labels, font sizing, literal text, and HTML/entity handling, including wrapped Swimlane edge labels and valid Class-note XML. [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247), [#250](https://github.com/Latias94/merman/pull/250)
- Restore Neo shadows in Sequence diagrams and GitGraph tag backgrounds, and prevent Sequence boxes from clipping at the diagram edges. [#255](https://github.com/Latias94/merman/pull/255)
- Honor Gantt `tickInterval`, Mindmap root padding/width, and Sequence numbering, participant visibility, and note alignment. When combining init directives, replace legacy `config` with explicit namespaces or frontmatter. [#252](https://github.com/Latias94/merman/pull/252)
- Preserve `%` in Flowchart node IDs and accept brackets in YAML frontmatter comments and block text. Separate Flowchart header directions from the diagram body with a newline or semicolon. [#241](https://github.com/Latias94/merman/pull/241), [#251](https://github.com/Latias94/merman/pull/251)

Review SVG snapshots after upgrading. See the [workspace changelog](../../../CHANGELOG.md) for the complete release.

## [0.9.2] - 2026-10-22

- Host themes keep C4 boundary contents and relationships visible, color Wardley anchors, and apply a coherent ZenUML palette. Diagram-selected Mermaid palettes retain matching text and fills. [#235](https://github.com/Latias94/merman/pull/235), [issue #229](https://github.com/Latias94/merman/issues/229), [issue #230](https://github.com/Latias94/merman/issues/230), [issue #231](https://github.com/Latias94/merman/issues/231), [issue #232](https://github.com/Latias94/merman/issues/232)
- Custom artifacts with native font support skip symbolic and pictorial fonts during implicit font and missing-glyph fallback; explicitly named fonts remain selectable. [#235](https://github.com/Latias94/merman/pull/235), [issue #228](https://github.com/Latias94/merman/issues/228)
- Look selectors and Block/Requirement source CSS are validated before SVG emission. SVG CSS cleanup rejects unterminated strings, comments, and URLs while preserving safe neighboring declarations. [#227](https://github.com/Latias94/merman/pull/227), [#235](https://github.com/Latias94/merman/pull/235), [#237](https://github.com/Latias94/merman/pull/237)
- Repeated Flowchart `classDef` declarations retain earlier styles, and unsupported Unicode characters preserve valid error spans and lexer progress. Malformed Pie/Info headers and empty XYChart sources behave consistently across parser and editor entrypoints. [#178](https://github.com/Latias94/merman/pull/178), [#227](https://github.com/Latias94/merman/pull/227), [#237](https://github.com/Latias94/merman/pull/237)
- Building wheels locates native libraries and UniFFI metadata in Cargo's configured target directory, including `CARGO_TARGET_DIR`. [#178](https://github.com/Latias94/merman/pull/178), [#237](https://github.com/Latias94/merman/pull/237)

The existing Presentation and binding options contracts are unchanged. Review regenerated SVG and exported-image snapshots; see the [workspace changelog](../../../CHANGELOG.md) for the complete maintenance release.

## [0.9.1] - 2026-10-18

- ER diagram SVG emission reuses prepared entity geometry, avoiding false missing-size errors on larger diagrams.
- Diagram lexers now make progress and honor cancellation while skipping malformed or empty input.
- Generated HTML label IDs are scoped to the diagram root so multiple diagrams can share a document.

## [0.9.0] - 2026-10-12

Version `0.9.0` retains UniFFI API 7 from 0.8.0. Keep the generated wrapper and native artifacts paired.

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

See the [package guide](README.md) and [upgrade guide](../../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md#binding-artifacts-and-callbacks).

## [0.8.0] - 2026-10-06

This entry consolidates the 0.7.0-to-0.8.0 package migration across the alphas. Version 0.8.0 was published on PyPI on 2026-10-06.

### Breaking changes

- Upgrade the generated Python wrapper and native library together to UniFFI API 7. Direct binding users use `binding_api_version_v7()` and `MermanOperationRequestV4`; do not substitute C ABI checks for UniFFI compatibility.
- Adopt Options JSON schema 2, analysis facts schema 2, and ASCII report schema 3. Rename viewport fields to `container_width` / `container_height`, move text/math selection under `environment`, and update layout/encoding/fallback decoders. Custom text measurers must return the tagged result shape for the requested measurement operation.

### Added

- Added reusable engines, cooperative operation controls, host measurement services, structured diagnostics/resource errors, runtime discovery, and richer ASCII layouts/projections. Read operations and capability availability from the loaded artifact.

### Changed

- The selected engine advances from Mermaid 11.15.0 in the previous stable line to Mermaid 12.1.0. Review ELK layout defaults, Redux/Neo appearance, SVG IDs, and geometry; request top-level `layout: dagre`, `theme: default`, and `look: classic` when those presentation choices are required. This does not preserve every old SVG byte.
- Default native artifacts include SVG, Cytoscape/ELK, ASCII, analysis, validation, and document analysis. Math, PNG/JPEG/PDF, and native runtime adapters require a matching custom artifact; query the loaded runtime catalog before using optional operations.

### Fixed

- Corrected C4 and ELK relationship routing/layering, Packet/XYChart presentation, partial theme overrides, and diagnostic locations; restored Usecase/ER styles and prevented malformed Unicode colors from panicking.

### Further reading

- Use the [package guide](README.md), [stable upgrade guide](../../../docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels), and [root changelog](../../../CHANGELOG.md). Native Rust benchmark observations do not establish Python transport performance.

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

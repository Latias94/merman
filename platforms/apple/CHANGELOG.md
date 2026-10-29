# Changelog

All notable changes to the Apple Swift package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [0.9.3] - Unreleased

- Flowchart preserves percent-containing node IDs, and YAML frontmatter no longer mistakes brackets in comments or block text for nested collections. [#241](https://github.com/Latias94/merman/pull/241), [#251](https://github.com/Latias94/merman/pull/251)
- Correct label sizing, configured fonts, literal text, and HTML/entity handling across SVG diagrams, including wrapped Swimlane edge labels and valid Class-note XML. Review updated output snapshots. [#245](https://github.com/Latias94/merman/pull/245), [#247](https://github.com/Latias94/merman/pull/247), [#250](https://github.com/Latias94/merman/pull/250)
- Apply Gantt `tickInterval`, Mindmap root padding and width settings, and Sequence numbering, participant visibility, and note alignment. Init directives follow Mermaid 12.1.0's filtering and merge order. [#252](https://github.com/Latias94/merman/pull/252)
- Host measurement adds `html-line-height` (operation 22) for literal decoded text. Keep generated Swift wrappers and native libraries paired; unsupported operations use fallback. UniFFI API 7 and Options JSON schema 2 remain unchanged. [#250](https://github.com/Latias94/merman/pull/250)

Native C SDK archives are a separate distribution from the Swift/XCFramework package and do not change its default capabilities. See the [workspace changelog](../../CHANGELOG.md).

## [0.9.2] - 2026-10-22

- Host themes keep C4 boundary contents and relationships visible, color Wardley anchors, and apply a coherent ZenUML palette. Diagram-selected Mermaid palettes retain matching text and fills. [#235](https://github.com/Latias94/merman/pull/235), [issue #229](https://github.com/Latias94/merman/issues/229), [issue #230](https://github.com/Latias94/merman/issues/230), [issue #231](https://github.com/Latias94/merman/issues/231), [issue #232](https://github.com/Latias94/merman/issues/232)
- Custom artifacts with native font support skip symbolic and pictorial fonts during implicit font and missing-glyph fallback; explicitly named fonts remain selectable. [#235](https://github.com/Latias94/merman/pull/235), [issue #228](https://github.com/Latias94/merman/issues/228)
- Look selectors and Block/Requirement source CSS are validated before SVG emission. SVG CSS cleanup rejects unterminated strings, comments, and URLs while preserving safe neighboring declarations. [#227](https://github.com/Latias94/merman/pull/227), [#235](https://github.com/Latias94/merman/pull/235), [#237](https://github.com/Latias94/merman/pull/237)
- Repeated Flowchart `classDef` declarations retain earlier styles, and unsupported Unicode characters preserve valid error spans and lexer progress. Malformed Pie/Info headers and empty XYChart sources behave consistently across parser and editor entrypoints. [#178](https://github.com/Latias94/merman/pull/178), [#227](https://github.com/Latias94/merman/pull/227), [#237](https://github.com/Latias94/merman/pull/237)

The existing Presentation and binding options contracts are unchanged. Review regenerated SVG and exported-image snapshots; see the [workspace changelog](../../CHANGELOG.md) for the complete maintenance release.

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

See the [package guide](README.md) and [upgrade guide](../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md#binding-artifacts-and-callbacks).

## [0.8.0] - 2026-10-06

This entry consolidates the 0.7.0-to-0.8.0 migration across the alphas. It prepares a matching Swift facade and XCFramework candidate; it does not announce artifact publication.

### Breaking changes

- Replace hand-written C bindings, raw callback pointers, and C ABI checks with generated UniFFI API 7 records and objects. Upgrade the source facade and every XCFramework slice together; use `MermanEngine(optionsJson:services:)` instead of `MermanReusableEngine` or old facade factories.
- Supply immutable text-measurement/icon services at engine construction. The Swift facade owns generated transport objects rather than public raw FFI structs; preserve typed busy/reentrant/cancellation errors.
- Adopt Options JSON schema 2, analysis facts schema 2, and ASCII report schema 3. Rename viewport fields to `container_width` / `container_height`, move text/math selection under `environment`, and update layout/encoding/fallback payload consumers.
- Use Swift 5.9 and iOS 14 / macOS 12 or newer. Follow the package guide for Xcode, local SwiftPM, and matching generated-source requirements.

### Added

- Added generic reusable operations, cooperative operation controls, runtime/presentation catalogs, typed resource/diagnostic details, and richer terminal plans.

### Changed

- The selected engine advances from Mermaid 11.15.0 in the previous stable line to the 12.1.0 candidate. Review ELK layout defaults, Redux/Neo appearance, SVG IDs, and geometry; request top-level `layout: dagre`, `theme: default`, and `look: classic` when those presentation choices are required. This does not preserve every old SVG byte.
- Default native artifacts include SVG, Cytoscape/ELK, ASCII, analysis, validation, and document analysis. Math, PNG/JPEG/PDF, and native runtime adapters require a matching custom artifact; query the loaded runtime catalog before using optional operations.

### Fixed

- Corrected C4 and ELK relationship routing/layering, Packet/XYChart presentation, partial theme overrides, and diagnostic locations; restored Usecase/ER styles and prevented malformed Unicode colors from panicking.

### Further reading

- Use the [package guide](README.md), [stable upgrade guide](../../docs/release/V070_TO_V080_UPGRADE_GUIDE.md#bindings-and-package-channels), and [root changelog](../../CHANGELOG.md). Native Rust benchmark observations do not establish Swift transport performance.

## [0.8.0-alpha.7] - 2026-09-30

### Added

- Added Mermaid 12 Agentflow and Usecase parsing and SVG rendering. Agentflow follows upstream beta syntax; neither family supports ASCII output.

### Changed

- Updated the engine from Mermaid 11.17.2 to 12.0.0, including ELK layout defaults and the new theme/look defaults for supported families. Refresh SVG snapshots; use top-level `layout: dagre`, `theme: default`, and `look: classic` when the previous presentation is needed. The bundled artifact already included ELK and retains its EPL-2.0 notices.
- Safe diagram-local theme variables and fonts are admitted from frontmatter and directives while host security policy remains authoritative.

### Breaking changes

- Added opt-in `auto` ASCII layout for bounded Flowchart and Sequence output, with a Compact retry before overflow handling. ASCII output reports now use schema `3`; update custom decoders for requested/effective layout and Compact-attempt fields.
- Direct UniFFI binding API advances to `7`. Regenerate Swift bindings and the native library together; use `bindingApiVersionV7()` to reject stale record layouts. The generic request remains `MermanOperationRequestV4`.

## [0.8.0-alpha.6] - 2026-09-02

This section describes alpha.6, whose matching XCFramework archive and checksum were attached to the GitHub Release on 2026-09-04.

### Breaking changes

- Advanced the direct UniFFI binding API to `6` because `MermanAsciiCapability` gained layout/width/encoding/fallback admission arrays and `MermanAsciiOutputPlan` gained schema-2 encoding. API 6 replaces `bindingApiVersionV5()` with `bindingApiVersionV6()` so stale generated Swift fails before decoding either changed record. Regenerate Swift and replace the XCFramework together.
- Renamed generic dispatch records to `MermanOperationRequestV4` and added optional `MermanOperationControl` values for cooperative cancellation and relative deadlines. Cancellation is a distinct generated error detail with its observed reason and phase; it is not a resource-limit failure.
- The default XCFramework now bundles SVG, both layout engines, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native runtime adapters. Generated helpers remain available for custom artifacts; the bundled library reports typed capability absence instead of carrying every optional backend.
- Analysis facts now use schema `2` and no longer include the Flowchart-only rich graph. Regenerate facts consumers for schema `2`; diagnostics remain on schema `1`.
- ASCII capability records now expose independent semantic coverage and primary projection fields, and rename `summaryFallback` to `structuredTextFallback`. Structured ASCII resource and diagnostic payloads follow the expanded six-phase renderer contract.

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
- Added Swift `MermanTextMeasureCallback`, `MermanTextMeasureRequest`, and `MermanTextMeasureResult` aliases for host text-measurement callbacks.

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

# Changelog

All notable changes to the Apple Swift package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [Unreleased]

### Breaking changes

- Replaced the prerelease ABI 2 text-measurement records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`lineCount`; upgrade the Swift sources and XCFramework together and update custom callbacks for operations `0...18`.
- Replaced the TextScan-capable document-facts payload shipped in `0.8.0-alpha.3` with the sole parser-only facts schema 1 contract; remove `fact_source: "text_scan"` handling, accept explicit unavailable bodies, and consume parser-backed rename policies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the alpha Flowchart ELK backend selector; update serialized `optionsJson` before upgrading.
- Moved binding JSON environment selectors to `environment.text_measurement` and `environment.math_renderer`, and theme variables to `host_theme.theme_variables`; remove legacy `layout.text_measurer`, `layout.math_renderer`, and `host_theme.themeVariables` keys before upgrading because they are now rejected.
- Removed underscore and shorthand binding enum aliases. Use the documented kebab-case values such as `resvg-safe`, `strip-existing-important`, `trusted-native`, and `unbounded-for-trusted-input`, plus generated host-theme preset names.
- Expanded the ABI 2 diagram-family capability record. Upgrade custom Swift decoders with the XCFramework; the canonical record now requires logical/render-model identities, parser/render flags, authoring header, and configuration namespace.

### Added

- Added generated Swift text-measurement operation/result-kind types with `requiredResultKind`.
- Added the generated `MermanResourceOptionsBuilder` and ABI 2 resource-profile/runtime descriptor
  so Apple callers can select `interactive`, `constrained`, `trusted-native`, or
  `unbounded-for-trusted-input` without duplicating limit tables.

### Changed

- Updated the XCFramework engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Host text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
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

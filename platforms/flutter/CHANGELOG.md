# Changelog

## Unreleased

### Breaking changes

- Replaced the prerelease ABI 2 text-measurement records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`lineCount`; upgrade the Dart package and bundled native artifacts together and update custom measurers for operations `0..18`.
- Replaced the TextScan-capable document-facts payload shipped in `0.8.0-alpha.3` with the sole parser-only facts schema 1 contract; remove `fact_source: "text_scan"` handling, accept explicit unavailable bodies, and consume parser-backed rename policies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the alpha Flowchart ELK backend selector; update serialized `optionsJson` before upgrading.
- Expanded the ABI 2 diagram-family capability record. Upgrade Dart constructor calls and custom strict JSON decoders with the bundled native artifacts; the canonical record now requires logical/render-model identities, parser/render flags, authoring header, and configuration namespace.

### Added

- Added generated text-measurement operation and result-kind enums.

### Changed

- Updated the bundled engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Host text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
- The pub package now carries the project license, source-provenance notice, and exact third-party license texts.

## 0.8.0-alpha.3

- Documented Flutter/Dart host text-measurement guidance for `MermanReusableEngine` callbacks.
- Added pub.dev metadata links and README compatibility notes for C ABI release discovery.
- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## 0.8.0-alpha.2

- Updated package metadata for the merman workspace `0.8.0-alpha.2` release.

## 0.8.0-alpha.1

- Updated package metadata for the merman workspace `0.8.0-alpha.1` release.

## 0.7.0

- Updated package metadata for the merman workspace `0.7.0` release.
- Added host theme preset and supported theme discovery through the bundled native bindings.

## 0.7.0-alpha.2

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## 0.7.0-alpha.1

- Initial experimental Flutter/Dart FFI package for the merman C ABI, including bundled native
  artifacts for Android, iOS, macOS, Windows, and Linux.

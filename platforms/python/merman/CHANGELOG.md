# Changelog

All notable changes to the Python package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## Unreleased

### Breaking changes

- Replaced the prerelease ABI 2 text-measurement callback records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`line_count`; upgrade the Python wheel and bundled native library together and update custom measurers for operations `0..18`.
- Replaced the TextScan-capable document-facts payload shipped in `0.8.0a3` with the sole parser-only facts schema 1 contract; remove `fact_source: "text_scan"` handling, accept explicit unavailable bodies, and consume parser-backed rename policies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the alpha Flowchart ELK backend selector; update any serialized `options_json` before upgrading.
- Expanded the ABI 2 diagram-family capability record. Upgrade custom strict JSON decoders with the bundled native library; the canonical record now includes logical/render-model identities, parser/render flags, authoring header, and configuration namespace.

### Added

- Added generated `ABI_VERSION` and `require_abi_version()` helpers plus exact text-measurement operation/result-kind enums.

### Changed

- Updated the bundled engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- Host text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
- Wheels now carry the project license, source-provenance notice, and exact third-party license texts in `.dist-info/licenses`.

## [0.8.0a3] - 2026-07-09

Corresponds to merman workspace release `0.8.0-alpha.3`.

### Added

- Added PyPI changelog metadata and README compatibility notes for UniFFI ABI and release discovery.
- Added UniFFI ABI 2 with reusable engines, diagram-family capability discovery, and host text-measurement callbacks that can be installed or cleared on reusable engines.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## [0.8.0a2] - 2026-06-13

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

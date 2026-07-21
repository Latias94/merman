# Changelog

All notable changes to the Android JNI package will be documented in this file.

The format is based on Keep a Changelog, and this package follows the merman workspace version.

## [Unreleased]

### Breaking changes

- Replaced the prerelease ABI 2 text-measurement JNI records in place: requests now carry both a routing phase and one of 19 exact operations, and handled callbacks must return that operation's tagged result kind instead of only `width`/`height`/`lineCount`; upgrade the Kotlin classes and `libmerman_ffi.so` together and update custom measurers for operations `0..18`.
- Replaced the TextScan-capable document-facts payload shipped in `0.8.0-alpha.3` with the sole parser-only facts schema 1 contract; remove `fact_source: "text_scan"` handling, accept explicit unavailable bodies, and consume parser-backed rename policies.
- Renamed binding option fields `viewport_width` and `viewport_height` to `container_width` and `container_height`, and removed the alpha Flowchart ELK backend selector; update serialized `optionsJson` before upgrading.
- Expanded the ABI 2 diagram-family capability JSON. Upgrade custom strict Kotlin/JSON decoders with the native library; the canonical record now includes logical/render-model identities, parser/render flags, authoring header, and configuration namespace.

### Added

- Added generated Kotlin text-measurement operation/result-kind constants.

### Changed

- Updated the native engine to the Mermaid 11.16 compatibility baseline, including source-backed Swimlane, Cynefin, Railroad, Wardley, and ZenUML behavior plus parser, layout, SVG, theme, Gantt, TreeView, and edge-routing fixes across existing families.
- JNI text-measurement failures, unsupported operations, and wrong-kind results now fall back per operation instead of invalidating the enclosing render.
- The AAR now carries the project license, source-provenance notice, and exact third-party license texts under `META-INF`.
- Android source builds now use the checked-in Gradle Wrapper and pinned JDK 17/NDK toolchain; the build helper can install the required NDK, assemble both published ABIs, and verify the completed AAR in one workflow.

## [0.8.0-alpha.3] - 2026-07-09

### Added

- Documented Android host text-measurement guidance for `MermanReusableEngine` callbacks.

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.3` release.

## [0.8.0-alpha.2] - 2026-06-13

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.2` release.

## [0.8.0-alpha.1] - 2026-06-10

### Changed

- Updated package metadata for the merman workspace `0.8.0-alpha.1` release.

## [0.7.0] - 2026-06-09

### Changed

- Updated package metadata for the merman workspace `0.7.0` release.
- Added host theme preset discovery through the Android JNI wrapper.

## [0.7.0-alpha.2] - 2026-06-08

### Changed

- Updated package metadata for the merman workspace `0.7.0-alpha.2` release.

## [0.7.0-alpha.1] - 2026-06-05

### Added

- Initial experimental Android JNI package for the merman C ABI.

# Merman For Apple Platforms

[![Swift 5.9+](https://img.shields.io/badge/Swift-5.9%2B-orange)](https://www.swift.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams from Swift on iOS and macOS without a WebView or JavaScript runtime. The Swift API wraps Merman's Rust engine in a binary XCFramework.

> **Alpha:** this repository currently provides a local Swift package, not a remotely resolvable SwiftPM dependency. Build or download the XCFramework for the same Merman release as the Swift sources; do not mix prerelease artifacts.

## Requirements

- Swift 5.9 or newer
- iOS 14 or newer, or macOS 12 or newer
- Xcode on macOS to build the XCFramework locally
- C ABI `2` Swift wrapper and matching `Merman.xcframework`

The wrapper checks the native ABI and exposed struct sizes during initialization. ABI 2 records can still be replaced in place during alpha development, so those checks do not make mixed prerelease releases supported.

## Add The Local Package

Build the binary target:

```sh
bash scripts/build-apple-xcframework.sh
```

Add the repository root as a local package in Xcode and link the `Merman` product. The root `Package.swift` resolves `platforms/apple/Merman.xcframework` by path.

Release workflows attach a versioned XCFramework archive and checksum to GitHub Releases. The current manifest intentionally has no remote `.binaryTarget(url:checksum:)`, so a GitHub asset alone is not a remote SwiftPM package declaration.

## Render A Diagram

```swift
import Merman

let engine = try MermanEngine()
let source = "flowchart TD\nA[Hello] --> B[World]"
let svg = try engine.renderSvg(source)
precondition(svg.hasPrefix("<svg"))
```

`MermanEngine` also exposes terminal rendering, semantic and layout JSON, validation, diagram/document analysis, parser facts, themes, lint metadata, ASCII support grades, and typed diagram-family capabilities.

Native calls are synchronous. Run non-trivial rendering away from latency-sensitive UI work. Failures are reported as `MermanError`, including ABI, struct-size, binding, JSON-decoding, and UTF-8 output errors.

## Options And Analysis

`optionsJson` follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```swift
let svg = try engine.renderSvg(
    source,
    optionsJson: #"{"svg":{"pipeline":"readable"}}"#
)
```

`analyzeJsonRaw` and `analyzeDocumentJsonRaw` return diagnostics schema `1`; `analyzeDocumentFactsJsonRaw` returns parser-backed facts schema `1`. These schema versions are independent of native ABI `2`. The removed TextScan alpha facts shape is not retained.

Pass full Markdown/MDX-like content and a URI to document analysis:

```swift
let factsJson = try engine.analyzeDocumentFactsJsonRaw(
    "```mermaid\n\(source)\n```",
    uri: "file:///workspace/README.md"
)
```

Use `diagramFamilyCapabilities()` and `asciiCapabilities()` instead of hard-coding support for a build profile or output format.

## Reusable Engines And Text Measurement

Use `MermanReusableEngine` for calls that share options and call `close()` when its native handle is no longer needed:

```swift
let reusable = try engine.reusableEngine()
defer { reusable.close() }
let svg = try reusable.renderSvg(source)
```

Merman owns a deterministic vendored text measurer by default. Keep it for background jobs, tests, and content generation. Native previews can install `MermanTextMeasureCallback` when layout must match Core Text or `NSAttributedString`; WebKit previews should use synchronously cached DOM/canvas measurements.

ABI 2 exposes 19 exact operations (`0...18`). `MermanTextMeasurementOperation.requiredResultKind` identifies the tagged result shape required for a handled request. Return `handled = 0` when an operation cannot be answered immediately and faithfully; invalid or wrong-kind results fall back for that operation.

Keep callback `userData` alive until the callback is cleared or the engine closes, and do not re-enter or mutate the same reusable engine from its callback. See the [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#apple-swift) for operation shapes and lifecycle rules.

## Verify Locally

```sh
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

The smoke covers SVG, terminal output, semantic/layout/analysis JSON, validation, metadata, ABI checks, and host measurement. The generated XCFramework is intentionally ignored by Git.

## Documentation And Releases

- [Apple binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)
- [Issue tracker](https://github.com/Latias94/merman/issues)

The supported distribution today is the repository-local package plus matching release assets. Remote SwiftPM resolution remains unavailable until a tagged manifest can name the immutable archive URL and checksum.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE),
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and
[`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) beside the Swift package or XCFramework.

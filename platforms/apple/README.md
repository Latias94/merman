# Merman Apple Package

Experimental Swift Package wrapper for iOS and macOS.

The package uses `platforms/apple/Merman.xcframework` as a binary target and exposes a Swift
`MermanEngine` over the C ABI.

## Build XCFramework

On macOS with Xcode:

```bash
bash scripts/build-apple-xcframework.sh
```

iOS-only:

```bash
bash platforms/ios/build-ios.sh
```

Generated `Merman.xcframework` is ignored by git.

## Swift API

```swift
import Merman

let engine = try MermanEngine()
let source = "flowchart TD\nA[Hello] --> B[World]"
let version = engine.packageVersion

let svg = try engine.renderSvg(
    source,
    optionsJson: #"{"svg":{"pipeline":"readable"}}"#
)
let semanticJson = try engine.parseJsonRaw(source)
let layoutJson = try engine.layoutJsonRaw(source)
let ascii = try engine.renderAscii(source)
let validation = try engine.validate(source)
let diagrams = try engine.supportedDiagrams()
let themes = try engine.supportedThemes()

do {
    _ = try engine.renderSvg(source, optionsJson: "{")
} catch MermanError.binding(_, let codeName, let message) {
    print("\(codeName): \(message)")
}
```

`MermanEngine` checks the native ABI version and FFI struct sizes during initialization. The package
version is read from the linked native library.
`optionsJson` follows the shared schema in
[`docs/bindings/OPTIONS_JSON.md`](../../docs/bindings/OPTIONS_JSON.md).

## Local Package Use

1. Build `platforms/apple/Merman.xcframework`.
2. Add this repository as a local Swift Package in Xcode.
3. Link product `Merman`.

## Smoke Example

After building the XCFramework, run the local SwiftPM smoke example:

```bash
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

The example lives in `platforms/apple/examples/smoke` and exercises SVG, ASCII, semantic JSON,
layout JSON, validation, and metadata through the Swift wrapper.

Release builds upload a zipped `Merman.xcframework` and checksum to GitHub Releases. Release
workflows do not move or force-update release tags.

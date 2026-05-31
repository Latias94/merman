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

do {
    _ = try engine.renderSvg(source, optionsJson: "{")
} catch MermanError.binding(_, let codeName, let message) {
    print("\(codeName): \(message)")
}
```

`MermanEngine` checks the native ABI version and FFI struct sizes during initialization. The package
version is read from the linked native library.

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

The example lives in `platforms/apple/examples/smoke` and exercises SVG, semantic JSON, and layout
JSON through the Swift wrapper.

Release builds upload a zipped `Merman.xcframework` to GitHub Releases and patch the release-tag
Swift package checksum for SwiftPM consumers.

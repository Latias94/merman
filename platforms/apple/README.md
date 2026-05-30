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
let svg = try engine.renderSvg("flowchart TD\nA[Hello] --> B[World]")
let semanticJson = try engine.parseJsonRaw("flowchart TD\nA[Hello] --> B[World]")
let layoutJson = try engine.layoutJsonRaw("flowchart TD\nA[Hello] --> B[World]")
```

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

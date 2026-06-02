# Apple Swift Package

Status: experimental scaffold.

The Apple wrapper uses a root SwiftPM package shape:

- root `Package.swift`
- `platforms/apple/Merman.xcframework` binary target
- Swift wrapper source under `platforms/apple/Sources/Merman`
- `scripts/build-apple-xcframework.sh`
- `platforms/ios/build-ios.sh` compatibility wrapper

## Build On macOS

```bash
bash scripts/build-apple-xcframework.sh
```

iOS-only:

```bash
bash platforms/ios/build-ios.sh
```

macOS-only:

```bash
bash scripts/build-apple-xcframework.sh --macos
```

The build script compiles `merman-ffi` as static libraries for iOS device, iOS simulator, and macOS,
then assembles `platforms/apple/Merman.xcframework`. It also writes a `MermanFFI` Clang module map
into each XCFramework header slice so SwiftPM can import the C ABI.

## Swift API

```swift
import Merman

let engine = try MermanEngine()
let svg = try engine.renderSvg("flowchart TD\nA[Hello] --> B[World]")
let ascii = try engine.renderAscii("flowchart TD\nA[Hello] --> B[World]")
let semanticJson = try engine.parseJsonRaw("flowchart TD\nA[Hello] --> B[World]")
let layoutJson = try engine.layoutJsonRaw("flowchart TD\nA[Hello] --> B[World]")
let validation = try engine.validate("flowchart TD\nA[Hello] --> B[World]")
let diagrams = try engine.supportedDiagrams()
```

The wrapper checks native ABI version and struct sizes on initialization. Native error payloads are
mapped to `MermanError.binding`.

## Smoke Example

```bash
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

## Verification Status

This scaffold was authored on Windows. `xcodebuild`, `lipo`, and SwiftPM build verification require
macOS with Xcode and are not run by the Windows platform gate. The Windows platform gate does check
that the Apple package files exist and that the Apple build scripts pass `bash -n` syntax validation.

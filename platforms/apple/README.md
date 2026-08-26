# Merman For Apple Platforms

Parse, analyze, lay out, and render Mermaid diagrams from Swift on iOS and macOS without a WebView or JavaScript runtime. The `Merman` SwiftPM product is a direct UniFFI binding packaged as a binary XCFramework plus generated Swift source.

> **Alpha:** the source tree uses direct UniFFI binding API `6`. Use the Swift source and XCFramework produced by the same Merman build. UniFFI rejects incompatible contract or API checksum pairs, but it does not compare Merman release versions when the generated interface is unchanged.

## Requirements

- Swift 5.9 or newer
- iOS 14 or newer, or macOS 12 or newer
- Xcode on macOS for local package use
- Python 3, Rust 1.95 with `rustup`, Cargo, and `lipo` only when building the XCFramework from source

For Swift 5.9 iOS integration, use Xcode 15.2 or newer. The SwiftPM 5.9 command-line client cannot select iOS slices from an XCFramework for `swift build --triple`; that command-line cross-build path requires SwiftPM 5.10 or newer.

## Add A Release XCFramework

Check out the source tree at the same release tag as the archive, then extract `Merman.xcframework-<tag>.zip` from [GitHub Releases](https://github.com/Latias94/merman/releases) so the framework is located at `platforms/apple/Merman.xcframework`. Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product.

The release archive contains the binary XCFramework and legal material. The matching checkout supplies the generated Swift facade and `Package.swift`; mixing tags is unsupported.

## Build The Local Package

```sh
bash scripts/build-apple-xcframework.sh
```

Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product. The root `Package.swift` resolves `platforms/apple/Merman.xcframework` by path. Release workflows attach a versioned XCFramework archive and checksum to GitHub Releases; the manifest deliberately does not currently declare a remote `.binaryTarget(url:checksum:)`.

## Render A Diagram

```swift
import Merman

let client = Merman()
let source = "flowchart TD\nA[Hello] --> B[World]"
let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try client.renderSvg(source: source, optionsJson: options)
precondition(svg.hasPrefix("<svg"))
```

`resourceOptionsJson` emits Options JSON schema `2`. Pass `nil` as the profile for a reusable request overlay that must inherit its constructor ceiling; generated override records accept only `MermanResourceOverrideId` values.

Use `MermanOperationRequestV4` and `client.execute(request:)` when the selected output is dynamic; put its options in the request's `optionsJson` field and pass `control: nil` when cancellation is not needed. The generated `MermanOperationResult` carries binary-safe bytes, media type, and typed operation metadata. For repeated work, construct `try MermanEngine(optionsJson:services:)` directly with baseline options and an optional immutable `MermanEngineServices` bundle. Per-operation options deep-merge over that baseline but cannot change the constructor-owned runtime policy. Call `close()` deterministically when an engine may retain foreign services; close is idempotent and retryable after busy or reentrant failures.

The generated binding API is 6. `MermanOperationRequestV4` remains the current request record name;
it does not identify the binding API version. API 6 adds ASCII layout/width/encoding/fallback
admission arrays and schema-2 output-plan encoding. `MermanError.Binding` includes an optional
`MermanDiagnosticErrorDetails` payload with a stable code, optional source span, field, and diagram
type. Use those structured details for parser and ASCII failures rather than matching display text.

Create `MermanOperationControl(timeoutMs:)` and attach it to a generic request when an editor or
preview host must stop stale work. The host may retain the same object and call `cancel()` from
another thread. Cancellation is cooperative and returns separate `MermanCancelledDetails`; it is
not a resource-limit failure. Hard termination of an opaque callback still requires worker or
process isolation.

Empty or omitted options select deterministic runtime state. The default XCFramework does not bundle native clock, time-zone, or random adapters. A custom source build may enable `native-runtime` and then pass `{"runtime_policy":"native"}`; the default artifact returns a typed unsupported-operation error for that request. Generic operation metadata records the selected `runtime_policy`.

Generated binding errors expose `MermanErrorKind`, an optional `capabilityId`, and optional typed
`MermanResourceErrorDetails`, `MermanDiagnosticErrorDetails`,
`MermanIconRegistryErrorDetails`, and `MermanCancelledDetails`. `.unknownOperation` has no
capability ID, `.missingCapability` preserves the stable descriptor ID required by the request,
resource failures preserve the stable cause (`ceiling` or `arithmetic_overflow`) plus typed limit
evidence, diagnostic failures preserve their stable code and optional source provenance, icon
registry failures preserve their structured registration evidence, and cancellation preserves its
reason and checkpoint phase without message parsing.

The default XCFramework includes semantic and layout JSON, analysis, validation, document analysis, ASCII, SVG, and both Cytoscape and ELK layouts. It omits math, PNG, JPEG, and PDF. The generated helpers remain available for custom current-contract libraries; the default artifact returns `.missingCapability` with `math`, `png`, `jpeg`, or `pdf` as appropriate. Check `runtimeCatalogJson()` rather than inferring support from package names or build flags, and decode `presentationCatalogJson()` when presenting theme or presentation-profile choices. Catalog IDs are open strings so a compatible native producer can add values without requiring a closed Swift enum update.

## Text Measurement

The default vendored measurer is appropriate for CI, server jobs, and deterministic output. Apple previews that must match their final font stack can start with `MermanEngineServices()`, call `withTextMeasurer(textMeasurer:)`, and pass the returned immutable bundle to the direct reusable-engine constructor. The original bundle remains unchanged, and the callback is immutable for the engine. Return `nil` for unhandled requests and avoid re-entering the engine during a measurement callback. Callback-free engines allow concurrent operations; callback engines serialize admission and report typed `busy` or `reentrantCall` errors without waiting. Only errors returned through UniFFI's generated callback trampoline are converted; callback code must not unwind across the generated FFI boundary. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#apple-swift) documents the protocol and lifecycle rules.

## Verify Locally

```sh
bash scripts/build-apple-xcframework.sh
swift build
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
git diff --exit-code -- platforms/apple/Sources/Merman/Generated
```

The final command proves that the checked-in UniFFI Swift projection matches the library used to build the XCFramework.

## Documentation And Releases

- [Apple binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [UniFFI binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) beside the Swift package or XCFramework.

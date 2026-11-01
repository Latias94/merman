# Merman For Apple Platforms

Parse, analyze, lay out, and render Mermaid diagrams from Swift on iOS and macOS without a WebView or JavaScript runtime. The `Merman` SwiftPM product is a direct UniFFI binding packaged as a binary XCFramework plus generated Swift source.

> **Binding contract:** Merman `0.9.3` uses direct UniFFI binding API `7`. Use the Swift source and XCFramework produced by the same Merman build. UniFFI rejects incompatible contract or API checksum pairs, but it does not compare Merman release versions when the generated interface is unchanged.

## Requirements

- Swift 5.9 or newer
- iOS 14 or newer, or macOS 12 or newer
- Xcode on macOS for local package use
- Python 3, Rust 1.95 with `rustup`, Cargo, and `lipo` only when building the XCFramework from source

For Swift 5.9 iOS integration, use Xcode 15.2 or newer. The SwiftPM 5.9 command-line client cannot select iOS slices from an XCFramework for `swift build --triple`; that command-line cross-build path requires SwiftPM 5.10 or newer.

This checkout documents the main development line, with UniFFI API `7` and unreleased
Options JSON schema `3`. Use matching generated Swift and native slices; see the
[changelog](CHANGELOG.md) and [upgrade guide](../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md#binding-artifacts-and-callbacks).

Compiled-theme APIs in this checkout are unreleased. Published `0.9.3` retains Options JSON
schema `2`; use its tagged guide even though both interfaces retain UniFFI API `7`.
See the [release boundary](../../docs/release/README.md#merman-093).

## Add the Complete Swift Package

For a release containing the new package, download `Merman-swiftpm-v<version>.zip`
and its `.checksum` from that release's assets. Extract it and add the resulting
`Merman-swiftpm-v<version>` directory as a local package in Xcode or with
`.package(path: "../Merman-swiftpm-v<version>")`. Link the `Merman` product.
The package includes matching generated Swift, resource declarations, XCFramework,
manifest and legal material. No Rust checkout or compiler is needed to consume it.

This complete package is a **main-line, unreleased delivery change**. The separately
named `Merman.xcframework-v<version>.zip` remains the binary-target input. Its checksum
differs from the complete Swift package's checksum. An emitted remote manifest is a
publisher handoff using that exact binary URL/digest; it is not an already hosted
SwiftPM repository. See [choose a prebuilt renderer](../../docs/release/PREBUILT_PRODUCTS.md).

## Install Published v0.9.3

Download the [0.9.3 archive](https://github.com/Latias94/merman/releases/download/v0.9.3/Merman.xcframework-v0.9.3.zip)
and [checksum](https://github.com/Latias94/merman/releases/download/v0.9.3/Merman.xcframework-v0.9.3.zip.checksum),
and use the matching `v0.9.3` checkout.

Check out the source tree at the same release tag as the archive, then extract `Merman.xcframework-<tag>.zip` from [GitHub Releases](https://github.com/Latias94/merman/releases) so the framework is located at `platforms/apple/Merman.xcframework`. Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product.

The published v0.9.3 archive contains the binary XCFramework and legal material.
Its matching checkout supplies the generated Swift facade and `Package.swift`;
mixing tags is unsupported. That artifact omits math and binary codecs. The new
complete package and math default do not change its bytes or capabilities.

## Build The Local Package

```sh
bash scripts/build-apple-xcframework.sh
```

Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product.
The root `Package.swift` resolves `platforms/apple/Merman.xcframework` by path and remains
the source-development manifest. After a complete build, produce a self-contained
distribution from a clean source tree:

```text
python3 scripts/apple_package.py --output-dir target/apple-package --version <workspace-version>
```

Partial `--ios` or `--macos` builds remain useful locally but cannot be packaged as
the complete distribution. Source builds also require
`rustup component add llvm-tools-preview` for distribution-copy cleanup.

## Render A Diagram

```swift
import Merman

let client = Merman()
let source = "flowchart TD\nA[Hello] --> B[World]"
let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try client.renderSvg(source: source, optionsJson: options)
precondition(svg.hasPrefix("<svg"))
```

`resourceOptionsJson` emits Options JSON schema `3`. Pass `nil` as the profile for a reusable request overlay that must inherit its constructor ceiling; generated override records accept only `MermanResourceOverrideId` values.

Use `MermanOperationRequestV4` and `client.execute(request:)` when the selected output is dynamic; put its options in the request's `optionsJson` field and pass `control: nil` when cancellation is not needed. The generated `MermanOperationResult` carries binary-safe bytes, media type, and typed operation metadata. For repeated work, construct `try MermanEngine(optionsJson:services:)` directly with baseline options and an optional immutable `MermanEngineServices` bundle. Per-operation options deep-merge over that baseline but cannot change the constructor-owned runtime policy. Call `close()` deterministically when an engine may retain foreign services; close is idempotent and retryable after busy or reentrant failures.

The generated binding API is 7. `MermanOperationRequestV4` remains the current request record name;
it does not identify the binding API version. API 7 preserves theme-authoring diagnostics and exposes `requestedLayoutProfile` and
`compactAttempted` in the schema-3 ASCII output plan. The effective `layoutProfile` remains
Canonical or Compact. Replace `bindingApiVersionV6()` with `bindingApiVersionV7()` when
regenerating source and the XCFramework together. API 6 introduced capability admission arrays
and explicit output encoding. `MermanError.Binding` includes an optional
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

The main-line default includes semantic and layout JSON, analysis, validation, document
analysis, ASCII, SVG, math, Cytoscape and ELK. PNG, JPEG and PDF remain typed absences.
For formulas, use `resourceOptionsJson(profile: .interactive, overrides: [])` when
constructing the engine; constrained policy still rejects opaque math. Compiled math
does not remove the need for host execution/memory limits. Swift's host measurement
callback is independent of the native C/WASI memory-font constructor, which this
package does not expose. Check `runtimeCatalogJson()` for the installed artifact's
actual capability IDs and `themeCatalogJson()` for presentation choices. Catalog IDs
remain open strings. Published v0.9.3 retains its math-free catalog.

## Text Measurement

The default deterministic, font-agnostic measurer is appropriate for CI, server jobs, and reproducible output. Apple previews that must match their final font stack can start with `MermanEngineServices()`, call `withTextMeasurer(textMeasurer:)`, and pass the returned immutable bundle to the direct reusable-engine constructor. The original bundle remains unchanged, and the callback is immutable for the engine. Return `nil` for unhandled requests and avoid re-entering the engine during a measurement callback. Callback-free engines allow concurrent operations; callback engines serialize admission and report typed `busy` or `reentrantCall` errors without waiting. Only errors returned through UniFFI's generated callback trampoline are converted; callback code must not unwind across the generated FFI boundary. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#apple-swift) documents the protocol and lifecycle rules.

## Verify Locally

```sh
bash scripts/build-apple-xcframework.sh
swift build
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
git diff --exit-code -- platforms/apple/Sources/Merman/Generated
```

The final command proves that the checked-in UniFFI Swift projection matches the library used to build the XCFramework.

The macOS smoke compares the shared theme catalog and authoring vectors, round-trips every preset recipe in that inventory, and imports complete Cyberpunk and edited Class recipes in fresh processes. It also checks that unsupported Class node widths retain a source-addressed diagnostic and rejected admission. Set `MERMAN_APPLE_THEME_SMOKE_OUTPUT` to retain each run's recipe files, SVGs, and operation metadata under a chosen directory. These checks do not qualify visual output, iOS execution, or the Swift 5.9 compiler floor.

The same smoke checks omitted, cleared, and transparent Class paints against source-owned styles.
Clear masks the earlier paint but retains an unsupported-route diagnostic; strict portability rejects
it. A deliberately absent font family remains a legal SVG reference with a fallback family. The
default artifact rejects embedded font resources before decoding, including at engine construction.
These checks do not prove that a browser or PDF renderer has the requested font or uses matching metrics.

## Documentation And Releases

- [Apple binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [UniFFI binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) beside the Swift package or XCFramework.

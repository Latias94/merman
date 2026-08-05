# Apple Swift Binding

Merman ships its Apple package as a direct UniFFI binding. Swift calls the
`merman-uniffi` component; it does not call the C ABI and does not maintain a
second Swift implementation of engine ownership, callbacks, or result buffers.

The local SwiftPM package contains:

- the root `Package.swift` product named `Merman`;
- a generated, checked-in `platforms/apple/Sources/Merman/Generated/Merman.swift`;
- a `MermanFFI` binary XCFramework module containing the matching UniFFI static library,
  generated C header, and module map; and
- `scripts/build-apple-xcframework.sh`, which regenerates the Swift binding before packaging.

## Build On macOS

```bash
bash scripts/build-apple-xcframework.sh
swift build
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

Use `--ios` or `--macos` to build a subset of slices. The script builds the release
library from the descriptor-owned `apple-uniffi-native` profile with its direct feature set, then
runs the local UniFFI generator with the same features plus `bindgen-smoke`. The generator must produce `Merman.swift`,
`MermanFFI.h`, and `MermanFFI.modulemap`; a missing library or generated artifact is a
hard failure. Each XCFramework slice contains the header and module map required by the
generated Swift source.

The generated Swift source is version controlled. After a local rebuild, check it with:

```bash
git diff --check -- platforms/apple/Sources/Merman/Generated
git diff --exit-code -- platforms/apple/Sources/Merman/Generated
```

The generated binding validates its UniFFI contract version and API checksums before the
first call. A mixed Swift source and native library fails with the generated binding's
explicit contract/checksum mismatch message; prerelease artifacts must always be upgraded
together.

## Swift API

```swift
import Merman

let source = "flowchart TD\nA[Hello] --> B[World]"
let engine = MermanEngine()

guard engine.bindingApiVersion() == 3 else {
    fatalError("unexpected Merman UniFFI binding API")
}

let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try engine.renderSvg(source: source, optionsJson: options)

let request = MermanOperationRequest(
    operationId: "png",
    source: source,
    uri: nil,
    optionsJson: options
)
let png = try engine.execute(request: request).data
```

`MermanEngine` owns one-shot operations. Use `MermanReusableEngine` when a group of
operations shares baseline options:

```swift
let reusable = try engine.reusableEngine(optionsJson: options)
let pdf = try reusable.renderPdf(source: source, optionsJson: nil)
```

The generic `execute` operation is the authoritative dispatch path. Its stable `operationId`
values, returned media type, and metadata are owned by Merman's capability descriptor. Named
methods such as `renderSvg`, `renderPng`, `renderJpeg`, and `renderPdf` are generated convenience
methods over that path. One-shot request options may select `runtime_policy`; reusable request
options deeply merge over the construction baseline but cannot replace its constructor-owned
runtime policy.

Generated `MermanError.Binding` values carry `kind: MermanErrorKind`, an optional `capabilityId`,
and optional `MermanResourceErrorDetails`. `.unknownOperation` has no capability ID;
`.missingCapability` preserves the exact descriptor capability required by the valid request.
Resource failures preserve the stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase,
actual value, effective maximum, and selected profile. Do not distinguish these cases by matching
the human-readable message.

## Capabilities And Limits

`engine.runtimeCatalogJson()` returns flat runtime catalog schema `1`. Its top-level transport and
package identity belong to the loaded UniFFI artifact; `capabilities` contains sorted current
capability, output, operation, system-adapter, and text-measurement IDs. `registry` and `resources`
describe the same artifact's diagram-family count and resource profiles. Consumers validate this
shape and its local relations, such as every output also being an operation and every reported
system adapter also being a capability. They must tolerate newly introduced stable IDs rather than
embedding a second copy of Merman's global vocabulary.

Use `resourceOptionsJson(profile:overrides:)` to build Options JSON schema `2`. `.constrained` is the recommended profile for untrusted or multi-tenant diagrams; pass `nil` for a reusable request overlay that must inherit its constructor ceiling. Override records accept only `MermanResourceOverrideId`, while the runtime catalog remains the complete source of truth for all limits. The complete resource decision table and error behavior are documented in [binding options](OPTIONS_JSON.md).

## Text Measurement

Merman uses its deterministic vendored measurer by default. A Swift UI that must match Core Text,
AppKit, or UIKit geometry can implement the generated `MermanTextMeasurer` protocol and pass it to
`engine.reusableEngineWithTextMeasurer(optionsJson:measurer:)`. The callback is immutable for that
engine; construct another engine to change it or return to the built-in measurer.

The callback receives the independent text-measurement protocol version `1`, not a C ABI record.
Return `nil` for a request that cannot be answered synchronously and faithfully; the corresponding
operation uses Merman's vendored fallback. Callback-free engines admit concurrent calls. Callback
engines serialize admission and return `.busy` to a competitor; same-engine entry from a callback
returns `.reentrantCall`. Only callback errors delivered through UniFFI's generated trampoline can
be converted to fallback. Callback implementations must not unwind across the generated FFI
boundary. See [host text measurement](HOST_TEXT_MEASUREMENT.md#apple-swift) for the operation and
lifecycle contract.

## Verification

The Apple smoke calls the generated public API against the built XCFramework. It verifies binding
API `3`, runtime catalog schema `1`, local capability relations, generic operation dispatch, reusable
operations, and SVG/PNG/JPEG/PDF output. CI also rebuilds the checked-in generated Swift binding,
so an API drift cannot pass by compiling an older hand-written facade.

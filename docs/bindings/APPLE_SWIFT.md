# Apple Swift Binding

Merman ships its Apple package as a direct UniFFI binding. Swift calls the
`merman-uniffi` component; it does not call the C ABI and does not maintain a
second Swift implementation of engine ownership, callbacks, or result buffers.

This guide owns Swift generation and XCFramework integration. The
[package README](../../platforms/apple/README.md) owns application examples, and the
[UniFFI contract](UNIFFI.md) owns shared API versions, output records, and lifecycle rules.

For binary installation, start with the complete local SwiftPM archive described in the
[prebuilt product guide](../release/PREBUILT_PRODUCTS.md). That main-line package includes
the facade and resource contract beside the XCFramework. Published v0.9.3 retains the older
matching-checkout installation and math-free artifact. The root manifest below is for source
development, not the layout of the extracted binary package.

The local SwiftPM package contains:

- the root `Package.swift` product named `Merman`;
- a generated, checked-in `platforms/apple/Sources/Merman/Generated/Merman.swift`;
- a `MermanFFI` binary XCFramework module containing the matching UniFFI static library,
  generated C header, and module map; and
- `scripts/build-apple-xcframework.sh`, which regenerates the Swift binding before packaging.

## Build On macOS

Install `rustup component add llvm-tools-preview` before building distribution copies.
The builder preserves the native build outputs, then removes delivery-only bitcode from
all thin staged libraries before assembling universal slices. Rust LTO settings are unchanged.
Main's recipe explicitly includes math; current Rust source defaults and convenience aggregates
keep math opt-in.

```bash
bash scripts/build-apple-xcframework.sh
swift build
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

Use `--ios` or `--macos` to build a subset of slices. The script builds the release
library from the descriptor-owned `apple-uniffi-native` profile with its direct feature set, then
runs the local UniFFI generator with only `binding-generation` against the built native library. The generator must produce `Merman.swift`,
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
let merman = Merman()

guard merman.bindingApiVersionV7() == 7 else {
    fatalError("unexpected Merman UniFFI binding API")
}

let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try merman.renderSvg(source: source, optionsJson: options)

let request = MermanOperationRequestV4(
    operationId: "ascii",
    source: source,
    uri: nil,
    optionsJson: options,
    control: nil
)
let ascii = try merman.execute(request: request).data
```

`Merman` owns discovery, metadata, and one-shot operations. Construct `MermanEngine` directly when
a group of operations shares baseline options or constructor services:

```swift
let engine = try MermanEngine(optionsJson: options, services: nil)
defer { try? engine.close() }
let diagnostics = try engine.analyzeJson(source: source, optionsJson: nil)
```

The generic `execute` operation is the authoritative dispatch path. Its stable `operationId`
values, returned media type, and metadata are owned by Merman's capability descriptor. Named
methods such as `renderSvg`, `renderPng`, `renderJpeg`, and `renderPdf` are generated convenience
methods over that path. One-shot request options may select `runtime_policy`; reusable request
options deeply merge over the construction baseline but cannot replace its constructor-owned
runtime policy.

Attach a `MermanOperationControl` to a generic request when the host needs a relative deadline or
must stop stale work from another thread:

```swift
let control = MermanOperationControl(timeoutMs: 250)
let request = MermanOperationRequestV4(
    operationId: "svg",
    source: source,
    uri: nil,
    optionsJson: options,
    control: control
)

// Retain `control` and call this from the host's invalidation path.
control.cancel()
```

Cancellation is cooperative. Parser, layout, SVG/ASCII emission, and export checkpoints observe
the shared control, but an opaque callback or encoder call may return before the next checkpoint.
Use worker or process isolation when the host requires hard preemption.

The main-line XCFramework supports SVG with math, both layout backends, ASCII, semantic/layout
operations, analysis, fixes, validation, and document analysis. This expanded recipe targets `0.10.0-alpha.1`;
published `0.9.3` retains the legacy math-free artifact described above. Main's default artifact
still omits the PNG, JPEG, and PDF exporters. The generated export helpers remain valid for custom
current-contract libraries; the default artifact returns `.missingCapability` with the required
descriptor ID for those formats. Math does not add native font discovery or the C/WASI memory-font
constructor service.

For admitted formula workloads, construct the engine with
`resourceOptionsJson(profile: .interactive, overrides: [])`. Main's constrained policy deliberately
rejects opaque math rendering even when math is compiled. Retain host execution and memory limits;
cooperative cancellation cannot preempt an opaque backend call. See the
[Swift product example](../release/PREBUILT_PRODUCTS.md#swift-installation).

Use `renderPngResult`, `renderJpegResult`, or `renderPdfResult` when effective output planning is
required. Switch on `outputPlan.kind`, inspect optional format payloads, and retain `rawJson` for
future kinds; see the [shared output contract](UNIFFI.md#public-model).

Generated `MermanError.Binding` values carry `kind: MermanErrorKind`, an optional `capabilityId`,
and optional `resource`, `diagnostic`, `iconRegistry`, and `cancellation` details. The corresponding
typed records are `MermanResourceErrorDetails`, `MermanDiagnosticErrorDetails`,
`MermanIconRegistryErrorDetails`, and `MermanCancelledDetails`. `.unknownOperation` has no capability ID;
`.missingCapability` preserves the exact descriptor capability required by the valid request.
Resource failures preserve the stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase,
actual value, effective maximum, and selected profile. Do not distinguish these cases by matching
the human-readable message. Cancellation remains a separate terminal class whose details expose
`reason` (`requested` or `deadline_exceeded`) and the checkpoint `phase`; it is never projected as
a resource limit and returns no partial output.

## Capabilities And Limits

`merman.runtimeCatalogJson()` returns flat runtime catalog schema `1`. Its top-level transport and
package identity belong to the loaded UniFFI artifact; `capabilities` contains sorted current
capability, output, operation, system-adapter, and text-measurement IDs. `registry` and `resources`
describe the same artifact's diagram-family count and resource profiles. Consumers validate this
shape and its local relations, such as every output also being an operation and every reported
system adapter also being a capability. They must tolerate newly introduced stable IDs rather than
embedding a second copy of Merman's global vocabulary.

Use `resourceOptionsJson(profile:overrides:)` to build Options JSON schema `3`. `.constrained` is the recommended profile for untrusted or multi-tenant diagrams; pass `nil` for a reusable request overlay that must inherit its constructor ceiling. Override records accept only `MermanResourceOverrideId`, while the runtime catalog remains the complete source of truth for all limits. The complete resource decision table and error behavior are documented in [binding options](OPTIONS_JSON.md).

## Text Measurement

Merman uses its deterministic, font-agnostic measurer by default. A Swift UI that must match Core Text,
AppKit, or UIKit geometry can implement the generated `MermanTextMeasurer` protocol, place it in
`MermanEngineServices`, and pass that value to the direct engine constructor:

```swift
let services = MermanEngineServices()
    .withTextMeasurer(textMeasurer: CoreTextMeasurer())
let engine = try MermanEngine(optionsJson: nil, services: services)
defer { try? engine.close() }
```

The callback is immutable for that engine; construct another engine to change it or return to the
built-in measurer.

The callback receives the independent text-measurement protocol version `1`, not a C ABI record.
Return `nil` for a request that cannot be answered synchronously and faithfully; the corresponding
operation uses Merman's deterministic fallback. Callback-free engines admit concurrent calls. Callback
engines serialize admission and return `.busy` to a competitor; same-engine entry or close from a
callback returns `.reentrantCall`. A busy or re-entrant close retains the engine and services for a
later retry. Only callback errors delivered through UniFFI's generated trampoline can be converted
to fallback. Callback implementations must not unwind across the generated FFI boundary. See
[host text measurement](HOST_TEXT_MEASUREMENT.md#apple-swift) for the operation and lifecycle
contract.

## Verification

The Apple smoke calls the generated public API against the built XCFramework. It checks SVG and
ASCII output, immutable icon and text-measurement services, resource and cancellation errors, and
deterministic close. Both one-shot and reusable consumers also execute the shared light/dark theme
materialization, support-query, and structured error vectors from
`crates/merman-theme-authoring-fixtures/fixtures/authoring-v1`. These fixtures are repository test
inputs, not resources bundled into the Swift library.
Owner-local Rust tests carry the broader catalog, output, and lifecycle contracts. CI also
rebuilds the checked-in generated Swift binding, so API drift cannot pass by compiling an older
hand-written facade.

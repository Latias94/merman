# Choose a Prebuilt Renderer

Choose the host where Merman will run, then the output your application needs. Cargo features
are for source builds; installing a prebuilt renderer does not require choosing them.

This guide describes the **main development line**. Its complete Apple package is unreleased.
Published `v0.9.3` keeps its original XCFramework installation and math-free Apple capability set.
Use the tagged documentation for an installed release. A source recipe or a successful candidate
build is not a published download.

## Recommended Paths

| Your application | Start here | Outputs and boundaries |
| --- | --- | --- |
| Swift on iOS or macOS | Complete `Merman-swiftpm-v<version>.zip` package | SVG and ASCII, all diagram families, math, Cytoscape and ELK; no PNG/JPEG/PDF |
| Native application with C/C++ interoperability | `sdk-native-full` for your target | SVG, PNG, JPEG, PDF and ASCII; math and ELK; caller-owned font bytes |
| Go, Python or another non-JavaScript sandbox host | `sdk-wasi-full` | SVG and PNG; math and ELK; one WASI reactor with a separate ABI |
| CLI, Rust, browser, Node, Python/UniFFI, Android or Flutter | [Package surfaces](PACKAGE_SURFACES.md) | Each existing package keeps its own supported outputs and capabilities |

Native C full targets macOS ARM64/x86-64, GNU Linux ARM64/x86-64 and Windows x86-64 MSVC.
It is not an iOS C SDK. Swift's XCFramework contains iOS ARM64, both iOS Simulator architectures,
and both macOS architectures. WASI targets `wasm32-wasip1`; choose a supported host runtime.

For a downloaded artifact, inspect its runtime catalog before offering optional operations in
your application. The catalog describes the loaded bytes. It also reports diagram families,
operation IDs, text-measurement providers and constructor services.

## Swift Installation

Extract the complete SwiftPM archive and add its `Merman-swiftpm-v<version>` directory as a local
package in Xcode. Link the `Merman` product. A Swift package can use:

```swift
.package(path: "../Merman-swiftpm-v<version>")
```

The archive includes `Package.swift`, generated Swift sources, resource declarations,
`Merman.xcframework` and notices. Keep these files together; a Rust checkout or compiler is
unnecessary. The separately named `Merman.xcframework-v<version>.zip` contains the binary and
legal material for binary-target consumers; it is not the complete local Swift package.

For formulas, select an interactive resource policy:

```swift
import Merman

let options = try resourceOptionsJson(profile: .interactive, overrides: [])
let engine = try MermanEngine(optionsJson: options, services: nil)
let svg = try engine.renderSvg(source: "flowchart TD\nA[\"$$x^2$$\"] --> B", optionsJson: nil)
try engine.close()
```

Constrained policy deliberately rejects opaque math rendering even when math is compiled.
For untrusted input, keep host execution and memory limits. The generated resource controls and
cooperative cancellation do not preempt an opaque dependency call.

A producer may supply `Merman-swiftpm-v<version>.Package.swift` as a remote-manifest handoff.
It references the exact binary ZIP URL and checksum and must be combined with the matching
`Sources/Merman` directory. It is not by itself a hosted SwiftPM repository. Public URL-based
installation is established only after those assets and a matching package repository are
published and an independent SwiftPM consumer resolves them.

See the [Apple package guide](../../platforms/apple/README.md) for examples and source builds.

## Native C and WASI Installation

Download the **full** archive for the transport and target you need, with its adjacent checksum,
from the selected version's release assets. Extract the entire archive. Native C includes C/C++
examples, CMake targets and pkg-config metadata. WASI includes real Go/wazero and
Python/wasmtime examples with pinned host dependencies.

Native C applications requesting math should construct their engine with
`{"resources":{"profile":"interactive"}}`. WASI selects interactive for omitted constructor
options. Both reject explicitly constrained math. Native C ABI 3 and WASI ABI 1 are independent;
main uses Options JSON schema 3. Do not mix native headers, wrappers or modules from different
source lines or releases.

See [SDK integration](../../distribution/sdk/README.md) for commands, platform requirements,
installed examples and qualification.

## Fonts and Output Formats

Complete SVG semantics do not imply every output codec or font service. Swift can use its existing
host text-measurement callback; it does not expose the native C/WASI memory-font constructor.
Native C and WASI can use caller-supplied font bytes for consistent measurement and painting.
Bundled Cousine is a limited fallback; provide suitable fonts for CJK and emoji.

SVG ordinarily retains text, and its viewer needs matching fonts. Math uses a separate embedded
formula-font set. Neither a package name nor selecting a codec guarantees the same text resources
in every viewer. See [host text measurement](../bindings/HOST_TEXT_MEASUREMENT.md) and
[raster/PDF output](../rendering/RASTER_OUTPUT.md).

## Advanced Choices

Choose `sdk-*-standard` when math and ELK are unnecessary, or `sdk-*-math` when formulas are
needed without ELK. These existing recipes retain their meanings. A reduced package returns a
typed missing-capability error for unsupported math rather than silently dropping the label.
Reduced packages still have their own notices; standard is not an MIT-only artifact.

Source consumers can select positive diagram and capability leaves. Rust defaults and convenience
aggregates remain math-free; add `math` explicitly. Expert selection is independent of the
recommended prebuilt product.

From a source checkout, discover recipes without invoking Rust:

```text
python3 scripts/artifact_profile_recipe.py --recommend swift --output svg --target-triple aarch64-apple-ios
python3 scripts/artifact_profile_recipe.py --recommend c --output pdf --target-triple x86_64-pc-windows-msvc
python3 scripts/artifact_profile_recipe.py --recommend wasi --output png
python3 scripts/artifact_profile_recipe.py --describe sdk-wasi-standard
python3 scripts/artifact_profile_recipe.py --list
```

An unsupported request fails with the supported outputs or targets. The query reads the exact
artifact descriptor; it does not fetch a release or require Cargo. Existing positional recipe
queries and build commands remain available to producer tooling.

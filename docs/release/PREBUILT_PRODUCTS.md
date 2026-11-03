# Install Merman for Your Host

Choose the host where Merman will run, then the output your application needs. Cargo features
are for source builds; installing a prebuilt renderer does not require choosing them.

This guide separates the published **0.10.0-alpha.1** channels from stable packages and prepared bindings. Choose a host below,
then follow that package's guide. Its wrapper and native/WASM library must belong to the same
release. Prebuilt installs do not need a Rust compiler on supported targets. The legacy Apple
package additionally requires a matching source checkout for its Swift facade and manifest.

## Published Versions

Registry metadata checked on 2026-11-03 shows these versions. Channels release separately;
main's workspace version does not establish a download on every registry.

| Channel | Stable version | Alpha version | Installation |
| --- | --- | --- | --- |
| Rust | `0.9.3` | `0.10.0-alpha.1` | `cargo add merman@0.10.0-alpha.1`; [alpha source feature choices](https://github.com/Latias94/merman/blob/v0.10.0-alpha.1/README.md) |
| CLI / standalone LSP | `0.9.3` | `0.10.0-alpha.1` | [Alpha release installers and archives](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.1) |
| Browser / Worker | `0.9.3` | `0.10.0-alpha.1` | `npm install @mermanjs/web@0.9.3` or `@mermanjs/web@alpha` |
| Native Node | `0.9.3` | Deferred | `npm install @mermanjs/node@0.9.3` |
| Python | `0.9.3` | Deferred | `python -m pip install merman==0.9.3` |
| Flutter / native Dart | `0.9.3` | Deferred | `flutter pub add merman:0.9.3` |
| Android | `0.9.3` AAR | Deferred | [Tagged Android guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/android/README.md) |
| Apple | `0.9.3` legacy XCFramework | Deferred | [Tagged Apple guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/apple/README.md) |
| Native C / WASI SDK | `0.9.3` | `0.10.0-alpha.1` | [Alpha SDK downloads and flavor choices](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.1) |

Web's five alpha packages use the `alpha` dist-tag; their stable `latest` is `0.9.3`.
Published Web full/render includes math. Published Node, Python, Flutter, Android and legacy Apple
products keep their version-specific, math-free capability selections. Main's expanded products
and new delivery handoffs below require their matching packages. Use documentation at the matching
tag for installed packages; do not mix main wrappers with maintenance binaries.

## Alpha Availability

The published `0.10.0-alpha.1` covers all 26 Rust crates, CLI/LSP, all 18 native C/WASI SDK
archives and all five Web npm packages. The [publication record](PUBLISH_ORDER.md#0100-alpha1-publication)
links the source-matched qualification and independently verified owner results. The GitHub Release
is a prerelease and does not replace stable `v0.9.3` as Latest.

Node, Python, Flutter, Android and Apple alpha delivery is deferred. Their main guides and install
examples describe prepared packages, not a registry download. Python's prepared version uses the
PEP 440 spelling `0.10.0a1` and covers five wheel targets, with GNU Linux ARM64/x86-64 at manylinux
2.28 (glibc `2.28+`). Published `0.9.3` keeps its original three-wheel coverage and Linux floor.
Native SVG math, thin Flutter assets and complete SwiftPM handoffs likewise require their matching
future packages.

## Recommended Paths

| Your application | Start here | Outputs and boundaries |
| --- | --- | --- |
| Browser or browser Worker | `@mermanjs/web`: [stable guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/web/README.md), [alpha guide](https://github.com/Latias94/merman/blob/v0.10.0-alpha.1/platforms/web/README.md) | Complete browser SDK; use `web-render` for SVG alone, or analysis/editor/ASCII task packages |
| Node, static-site generation or server rendering | [`@mermanjs/node`](https://github.com/Latias94/merman/blob/v0.9.3/platforms/node/README.md) | Platform package selected automatically; `node-wasm` is an explicit portable alternative |
| Python | [`merman` wheel](https://github.com/Latias94/merman/blob/v0.9.3/platforms/python/merman/README.md) | pip/uv selects a matching native wheel; no Rust for ordinary supported installs |
| Flutter or native Dart | [`merman` on pub.dev](https://github.com/Latias94/merman/blob/v0.9.3/platforms/flutter/README.md) | Stable package bundles its native libraries; no separate target download |
| Kotlin on Android | [Release AAR](https://github.com/Latias94/merman/blob/v0.9.3/platforms/android/README.md) | Matching Kotlin/JNI in one AAR; add the local AAR to Gradle |
| Swift on iOS or macOS | [Stable Apple guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/apple/README.md) | Matching `v0.9.3` checkout supplies Swift sources and `Package.swift`; add the published XCFramework |
| Native application with C/C++ interoperability | `sdk-native-full` for your target | SVG, PNG, JPEG, PDF and ASCII; math and ELK; caller-owned font bytes |
| Go, Python or another non-JavaScript sandbox host | `sdk-wasi-full` | SVG and PNG; math and ELK; one WASI reactor with a separate ABI |
| Rust, CLI/LSP, Rustdoc, Typst or editor integration | [Package surfaces](PACKAGE_SURFACES.md) | Source crates and independent task products keep their existing installation routes |

Web full/render includes math in the published versions above. Stable Swift, Android, Python,
Flutter and Node products omit math and PNG/JPEG/PDF encoders. Native C/WASI SDK flavors have
their own output and font services. Dedicated analysis/editor/ASCII packages retain their task
scope. Check the loaded runtime catalog and the matching version's guide for available operations.

The recommended package should be enough to render ordinary documents. Reduced SDK flavors
and source feature selection are advanced choices, rather than installation prerequisites.

Native C full targets macOS ARM64/x86-64, GNU Linux ARM64/x86-64 and Windows x86-64 MSVC.
It is not an iOS C SDK. Swift's XCFramework contains iOS ARM64, both iOS Simulator architectures,
and both macOS architectures. WASI targets `wasm32-wasip1`; choose a supported host runtime.

For a downloaded artifact, inspect its runtime catalog before offering optional operations in
your application. The catalog describes the loaded bytes. It also reports diagram families,
operation IDs, text-measurement providers and constructor services.

## Swift Installation

For published Apple `0.9.3`, check out `v0.9.3` and download
[`Merman.xcframework-v0.9.3.zip`](https://github.com/Latias94/merman/releases/download/v0.9.3/Merman.xcframework-v0.9.3.zip)
with its adjacent SHA-256 file from the [stable Release](https://github.com/Latias94/merman/releases/tag/v0.9.3).
Extract the framework to `platforms/apple/Merman.xcframework` in that matching checkout.
Add the repository root as a local package in Xcode and link the `Merman` product.
A Swift package can use:

```swift
.package(path: "../merman")
```

The checkout supplies `Package.swift` and generated Swift sources; the ZIP supplies the binary
and legal material. A Rust compiler is unnecessary when using the release XCFramework.
Keep the source and binary at the same tag:

```swift
import Merman

let client = Merman()
let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try client.renderSvg(source: "flowchart TD\nA[Hello] --> B[World]", optionsJson: options)
```

Apple `0.9.3` has no math capability, complete SwiftPM archive or public remote package repository.
See the [tagged Apple guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/apple/README.md)
for examples and source builds. The prepared main delivery is described separately below.

## Native C and WASI Installation

Download the **full** archive for the transport and target you need, with its adjacent checksum,
from the selected version's release assets. Extract the entire archive. Native C includes C/C++
examples, CMake targets and pkg-config metadata. WASI includes real Go/wazero and
Python/wasmtime examples with pinned host dependencies.

Native C applications requesting math should construct their engine with
`{"resources":{"profile":"interactive"}}`. WASI selects interactive for omitted constructor
options. Alpha `0.10.0-alpha.1` rejects explicitly constrained math and uses Options JSON schema 3.
Stable `0.9.3` retains its schema-2 resource behavior, including qualified small formulas under
constrained requests. Native C ABI 3 and WASI ABI 1 are independent. Do not mix native headers,
wrappers or modules from different source lines or releases.

See the [alpha SDK guide](https://github.com/Latias94/merman/blob/v0.10.0-alpha.1/distribution/sdk/README.md)
or [stable SDK guide](https://github.com/Latias94/merman/blob/v0.9.3/distribution/sdk/README.md)
for the selected version's commands, platform requirements, installed examples and qualification.

## Fonts and Output Formats

Complete SVG semantics do not imply every output codec or font service. Swift can use its existing
host text-measurement callback; it does not expose the native C/WASI memory-font constructor.
Native C and WASI can use caller-supplied font bytes for consistent measurement and painting.
Bundled Cousine is a limited fallback; provide suitable fonts for CJK and emoji.

SVG ordinarily retains text, and its viewer needs matching fonts. Math uses a separate embedded
formula-font set. Neither a package name nor selecting a codec guarantees the same text resources
in every viewer. See [host text measurement](../bindings/HOST_TEXT_MEASUREMENT.md) and
[raster/PDF output](../rendering/RASTER_OUTPUT.md).

## Platform and Download Choices

Python wheel compatibility includes OS, CPU and libc, not just the Python version. Main
targets GNU Linux ARM64/x86-64, macOS ARM64/x86-64 and Windows x86-64; its Linux baseline
must be qualified from the final repaired wheel. Published `0.9.3` has three wheels, including
`manylinux_2_35_x86_64`, and has no source distribution. An unsupported target does not gain
support merely by installing Rust; see the explicit source-helper route in the Python guide.

Node native packages distinguish glibc from musl. Keep optional dependencies enabled and
install on the deployment target; copying macOS `node_modules` to Linux does not select a
Linux binary. Main adds Linux ARM64 coverage; it is not present in published `0.9.3` native
packages. Node-WASM is a separate explicit choice and is not the browser transport.

Stable Flutter bundles the native libraries in the pub package. Native Dart support does not
imply Flutter Web support. The prepared main hook and its offline requirements are described below.

Browser code initializes once per realm. Use the package's default WASM URL or explicit
self-hosted loading and retain matching JS/glue/WASM. A Worker owns its own initialized
runtime and lifetime. Server-side Node code should use the Node entry point.

Download size, selected native-library size and the final linked application contribution
are different measurements. An XCFramework ZIP contains multiple device, simulator and
desktop slices; its total size is not an iPhone application's installed-size contribution.

## Prepared Binding Products

These main-source bindings have not shipped as `0.10.0-alpha.1`. Their guides document prepared
APIs and delivery handoffs, rather than currently installable registry versions:

| Prepared binding | Change requiring its matching future package |
| --- | --- |
| [Node](../../platforms/node/README.md) | SVG math and native Linux ARM64 packages; nine-member npm group |
| [Python](../../platforms/python/merman/README.md) | SVG math, five wheel targets and manylinux 2.28 |
| [Flutter](../../platforms/flutter/README.md) | Thin facade with verified native downloads selected at build time; prepare exact targets or use the local artifact route for offline builds |
| [Android](../../platforms/android/README.md) | SVG math and a complete Maven repository ZIP/verification handoff; Central rollout remains separate |
| [Apple](../../platforms/apple/README.md) | SVG math, complete local SwiftPM ZIP and thin remote delivery tree; public remote installation requires a hosted, consumer-qualified package repository |

These ordinary SVG products select math, Cytoscape and ELK explicitly. They do not acquire
PNG/JPEG/PDF encoders or the C/WASI memory-font service. Their main resource policy rejects
explicitly constrained math; resource controls and cooperative cancellation do not preempt opaque
dependency calls. Use the released version's guide and runtime catalog until its replacement ships.

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
python3 scripts/artifact_profile_recipe.py --recommend python --target-triple aarch64-unknown-linux-gnu
python3 scripts/artifact_profile_recipe.py --recommend flutter --target-triple aarch64-apple-ios-sim
python3 scripts/artifact_profile_recipe.py --recommend android --target-triple aarch64-linux-android
python3 scripts/artifact_profile_recipe.py --recommend web
python3 scripts/artifact_profile_recipe.py --describe sdk-wasi-standard
python3 scripts/artifact_profile_recipe.py --list
```

An unsupported request fails with the supported outputs or targets. The query reads the exact
artifact descriptor; it does not fetch a release or require Cargo. Existing positional recipe
queries and build commands remain available to producer tooling.

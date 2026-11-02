# Install Merman for Your Host

Choose the host where Merman will run, then the output your application needs. Cargo features
are for source builds; installing a prebuilt renderer does not require choosing them.

This guide separates the **main development line** from published packages. Choose a host below,
then follow that package's guide. Its wrapper and native/WASM library must belong to the same
release. Ordinary prebuilt use does not need a Rust checkout or compiler on supported targets.

## Published Versions

Registry metadata checked on 2026-11-01 shows these versions. Channels release separately;
main's workspace version does not establish a download on every registry.

| Channel | Published version | Installation |
| --- | --- | --- |
| Rust | `0.9.3` | `cargo add merman`; [source feature choices](../../README.md) |
| CLI / standalone LSP | `0.9.3` | [Release installers and archives](../../crates/merman-cli/README.md) |
| Browser / Worker | `0.9.2` | `npm install @mermanjs/web@0.9.2` |
| Native Node | `0.9.2` | `npm install @mermanjs/node@0.9.2` |
| Python | `0.9.2` | `python -m pip install merman==0.9.2` |
| Flutter / native Dart | `0.9.2` | `flutter pub add merman:0.9.2` |
| Android | `0.9.2` AAR | [Tagged Android guide](https://github.com/Latias94/merman/blob/v0.9.2/platforms/android/README.md) |
| Apple | `0.9.3` legacy XCFramework | [Tagged Apple guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/apple/README.md) |
| Native C / WASI SDK | `0.9.3` | [Tagged SDK guide](https://github.com/Latias94/merman/blob/v0.9.3/distribution/sdk/README.md) |

Published Web full/render `0.9.2` includes math. Published Node, Python, Flutter, Android and
legacy Apple products have their version-specific, math-free capability selections. Main's
expanded products and new delivery handoffs below are **unreleased**. Use documentation at
the matching tag for installed packages; do not mix main wrappers with maintenance binaries.

## Recommended Paths

| Your application | Start here | Outputs and boundaries |
| --- | --- | --- |
| Browser or browser Worker | [`@mermanjs/web`](../../platforms/web/README.md) | Complete browser SDK; use `web-render` for SVG alone, or analysis/editor/ASCII task packages |
| Node, static-site generation or server rendering | [`@mermanjs/node`](../../platforms/node/README.md) | Platform package selected automatically; `node-wasm` is an explicit portable alternative |
| Python | [`merman` wheel](../../platforms/python/merman/README.md) | pip/uv selects a matching native wheel; no Rust for ordinary supported installs |
| Flutter or native Dart | [`merman` on pub.dev](../../platforms/flutter/README.md) | One Dart facade with target-aware native acquisition; see the offline/local artifact route |
| Kotlin on Android | [Maven publication](../../platforms/android/README.md) | AAR with matching Kotlin/JNI and dependency metadata; repository ZIP before Central rollout |
| Swift on iOS or macOS | [SwiftPM package](../../platforms/apple/README.md) | Complete local ZIP or remote delivery handoff; public remote installation awaits a hosted package repository |
| Native application with C/C++ interoperability | `sdk-native-full` for your target | SVG, PNG, JPEG, PDF and ASCII; math and ELK; caller-owned font bytes |
| Go, Python or another non-JavaScript sandbox host | `sdk-wasi-full` | SVG and PNG; math and ELK; one WASI reactor with a separate ABI |
| Rust, CLI/LSP, Rustdoc, Typst or editor integration | [Package surfaces](PACKAGE_SURFACES.md) | Source crates and independent task products keep their existing installation routes |

Main's ordinary SVG rendering products select math, Cytoscape and ELK explicitly. Dedicated
analysis/editor/ASCII packages retain their task scope. Swift, Android, Python, Flutter and
Node do not thereby acquire PNG/JPEG/PDF encoders or the C/WASI memory-font service. Check
the loaded runtime catalog and your binding guide for the actual available operations.

The recommended package should be enough to render ordinary documents. Reduced SDK flavors
and source feature selection are advanced choices, rather than installation prerequisites.

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

A producer can use `Merman-swiftpm-remote-v<version>.zip` as the complete thin delivery-tree
handoff. It includes the matching Swift facade, resources and legal material, with a manifest
that references the separate binary ZIP by its exact HTTPS URL and checksum. Publish that
binary and commit the unchanged delivery tree to the selected SwiftPM repository. Public
URL-based installation is established only after an independent consumer resolves that
repository and downloads the matching binary. The standalone `Package.swift` is retained as
an advanced handoff; users of the complete tree do not need to assemble its sources manually.

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

## Platform and Download Choices

Python wheel compatibility includes OS, CPU and libc, not just the Python version. Main
targets GNU Linux ARM64/x86-64, macOS ARM64/x86-64 and Windows x86-64; its Linux baseline
must be qualified from the final repaired wheel. Published `0.9.2` has three wheels, including
`manylinux_2_35_x86_64`, and has no source distribution. An unsupported target does not gain
support merely by installing Rust; see the explicit source-helper route in the Python guide.

Node native packages distinguish glibc from musl. Keep optional dependencies enabled and
install on the deployment target; copying macOS `node_modules` to Linux does not select a
Linux binary. Main adds Linux ARM64 coverage; it is not present in published `0.9.2` native
packages. Node-WASM is a separate explicit choice and is not the browser transport.

Flutter's target-aware hook moves native acquisition into the build. A prepared pub cache
alone does not guarantee an offline Android/iOS build: prepare the exact native target and
Apple SDK slice, or configure the verified local artifact route. Application runtime use
does not download code. Native Dart support does not imply Flutter Web support.

Browser code initializes once per realm. Use the package's default WASM URL or explicit
self-hosted loading and retain matching JS/glue/WASM. A Worker owns its own initialized
runtime and lifetime. Server-side Node code should use the Node entry point.

Download size, selected native-library size and the final linked application contribution
are different measurements. An XCFramework ZIP contains multiple device, simulator and
desktop slices; its total size is not an iPhone application's installed-size contribution.

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

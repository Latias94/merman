# Install Merman for Your Host

Choose the host where Merman will run, then the output your application needs. Cargo features
are for source builds; installing a prebuilt renderer does not require choosing them.

Choose the published version for your host below. Keep the wrapper and native or WASM library on
the same release. Supported prebuilt installs need no Rust compiler. The
[upgrade index](README.md#workspace-upgrades) covers Rust, CLI, and binding changes by version.

## Published Versions

The versions below were verified on 2026-11-22. Select an exact version for reproducible installs.

| Channel | Published version | Installation |
| --- | --- | --- |
| Rust | `0.11.0` | `cargo add merman@0.11.0` |
| CLI / standalone LSP | `0.11.0` | [Installers and archives](https://github.com/Latias94/merman/releases/tag/v0.11.0) |
| Browser / Worker | `0.11.0` | `npm install --save-exact @mermanjs/web@0.11.0` |
| Native Node / Node-WASM | `0.11.0` | `npm install --save-exact @mermanjs/node@0.11.0` or `npm install --save-exact @mermanjs/node-wasm@0.11.0` |
| Python | `0.10.0` | `python -m pip install --only-binary=:all: merman==0.10.0` |
| Flutter / native Dart | `0.10.0` | `flutter pub add 'merman:0.10.0'` |
| Android | `0.10.0` | [Local Maven ZIP or AAR](#android-installation) |
| Apple | `0.10.0` | [Complete local SwiftPM / XCFramework](#swift-installation) |
| Native C / WASI SDK | `0.10.0` | [SDK downloads](https://github.com/Latias94/merman/releases/tag/v0.10.0) and [flavor choices](#native-c-and-wasi-installation) |

The [0.11.0 upgrade notes](README.md#merman-0110) cover layout, fonts, and binding changes.
Use the [0.10.0 upgrade guide](V093_TO_V0100_UPGRADE_GUIDE.md) for earlier migrations from
`0.9.x` and the `0.10.0` alphas. Each registry and package manager follows its own release schedule.

## Recommended Paths

| Your application | Start here | Outputs and boundaries |
| --- | --- | --- |
| Browser or browser Worker | [Browser SDK](https://github.com/Latias94/merman/blob/v0.11.0/platforms/web/README.md) | Complete browser SDK or focused SVG, analysis, editor, and ASCII packages |
| Node, static-site generation or server rendering | [`@mermanjs/node`](https://github.com/Latias94/merman/blob/v0.11.0/platforms/node/README.md) | Automatic native package selection with explicit Node-WASM available separately |
| Python | [`merman` wheel](https://github.com/Latias94/merman/blob/v0.10.0/platforms/python/merman/README.md) | pip and uv select a matching native wheel |
| Flutter or native Dart | [`merman` on pub.dev](https://pub.dev/packages/merman/versions/0.10.0) | Downloads and verifies the build target's library, with local assets for offline builds |
| Kotlin on Android | [Local Maven repository](#android-installation) | Maven ZIP retains AAR metadata and transitive dependencies |
| Swift on iOS or macOS | [Complete local SwiftPM package](#swift-installation) | ZIP includes the facade, resources, manifest, and XCFramework |
| Native application with C/C++ interoperability | `sdk-native-full` for your target | SVG, PNG, JPEG, PDF and ASCII; math and ELK; caller-owned font bytes |
| Go, Python or another non-JavaScript sandbox host | `sdk-wasi-full` | SVG and PNG; math and ELK; one WASI reactor with a separate ABI |
| Rust, CLI/LSP, Rustdoc, Typst or editor integration | [Package surfaces](PACKAGE_SURFACES.md) | Source crates and independent task products keep their existing installation routes |

The `0.11.0` Web full/render packages and both Node transports include SVG math, as do the
`0.10.0` Swift, Android, Python, and Flutter products. These native bindings do not include PNG/JPEG/PDF
encoders or native C/WASI's memory-font service. Native C/WASI SDK flavors have their own output
and font services. Dedicated analysis/editor/ASCII packages retain their task
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

Download the complete
[`Merman-swiftpm-v0.10.0.zip`](https://github.com/Latias94/merman/releases/download/v0.10.0/Merman-swiftpm-v0.10.0.zip)
and its
[SHA-256 checksum](https://github.com/Latias94/merman/releases/download/v0.10.0/Merman-swiftpm-v0.10.0.zip.checksum).
Verify the checksum, extract the archive and add the resulting directory as a local package in
Xcode. Link the `Merman` product. A Swift package can use:

```swift
.package(path: "../Merman-swiftpm-v0.10.0")
```

The archive includes matching Swift sources, resources, manifest, XCFramework and legal material.
No source checkout or Rust compiler is needed. Do not substitute the separately named
`Merman.xcframework-v0.10.0.zip`: that smaller archive supplies only the binary target.

```swift
import Merman

let client = Merman()
let options = try resourceOptionsJson(profile: .interactive, overrides: [])
let svg = try client.renderSvg(source: "flowchart TD\nA[Hello] --> B[World]", optionsJson: options)
```

The release also provides a thin remote package tree for publishers. A public Git-URL installation
requires its own hosted and qualified repository; downloading that tree does not make the source
repository URL a working binary package. See the [Apple guide](../../platforms/apple/README.md).

For stable `0.9.3`, use the [legacy Apple instructions](../../platforms/apple/README.md#install-published-v093):
its XCFramework needs the matching source checkout for the facade and `Package.swift`, and omits
math. Keep all inputs at `v0.9.3`.

## Android Installation

Download the complete
[`merman-android-0.10.0-maven.zip`](https://github.com/Latias94/merman/releases/download/v0.10.0/merman-android-0.10.0-maven.zip)
and its
[SHA-256 sidecar](https://github.com/Latias94/merman/releases/download/v0.10.0/merman-android-0.10.0-maven.zip.sha256).
Verify the checksum and extract it to a local directory. The [Gradle example](../../platforms/android/README.md#resolve-a-candidate-maven-publication)
resolves `io.merman:merman-android:0.10.0` with these properties:

```text
-PmermanRepository=/absolute/path/to/extracted/repository
-PmermanGroup=io.merman
-PmermanVersion=0.10.0
```

The repository path is the extraction root containing `io/merman/merman-android/`. Gradle then
reads the POM/module metadata and resolves the Kotlin dependency along with the AAR. A standalone
AAR is also available from the same Release. This is local Maven installation; Maven Central
publication remains separate.

## Native C and WASI Installation

Download the **full** archive for the transport and target you need, with its adjacent checksum,
from the selected version's release assets. Extract the entire archive. Native C includes C/C++
examples, CMake targets and pkg-config metadata. WASI includes real Go/wazero and
Python/wasmtime examples with pinned host dependencies.

Native C applications requesting math should construct their engine with
`{"resources":{"profile":"interactive"}}`. WASI selects interactive for omitted constructor
options. Both `0.10.0` SDKs use Options JSON schema `3`. A WASI request with an explicitly
constrained profile can reject math. Use matching C headers, wrappers, and modules for Native C
ABI `3` and WASI ABI `1`.

See the [0.10.0 SDK guide](https://github.com/Latias94/merman/blob/v0.10.0/distribution/sdk/README.md)
for commands, platform requirements, installed examples, and qualification.

## Fonts and Output Formats

For CLI PNG, JPEG, and PDF, the official Windows archive uses platform font discovery by default.
The macOS and Linux archives and ordinary Cargo installs default to bounded discovery. For a fixed
font set and quicker startup, pass `--font-file` for each required file and `--font-system off`.
Ordinary Mermaid-to-SVG rendering uses deterministic measurement by default. [CLI font guide](../../crates/merman-cli/README.md#explicit-fonts)

Complete SVG semantics do not imply every output codec or font service. Swift can use its existing
host text-measurement callback; it does not expose the native C/WASI memory-font constructor.
Native C and WASI can use caller-supplied font bytes for consistent measurement and painting.
Bundled Cousine is a limited fallback; provide suitable fonts for CJK and emoji.

SVG ordinarily retains text, and its viewer needs matching fonts. Math uses a separate embedded
formula-font set. Neither a package name nor selecting a codec guarantees the same text resources
in every viewer. See [host text measurement](../bindings/HOST_TEXT_MEASUREMENT.md) and
[raster/PDF output](../rendering/RASTER_OUTPUT.md).

## Platform and Download Choices

Python `0.10.0` wheels cover GNU Linux ARM64/x86-64, macOS ARM64/x86-64, and Windows x86-64.
GNU Linux wheels use manylinux 2.28 and require glibc `2.28+`. The Python guide documents
the separate source-helper route for checkout builds.

Node requires Node.js `22+`; GNU Linux native packages require glibc `2.31+`. Native packages
distinguish glibc from musl and support Linux ARM64 and x86-64. Keep optional dependencies enabled
and install on the deployment target. For the portable transport, install `@mermanjs/node-wasm@0.11.0`
and follow the [Node-WASM guide](https://github.com/Latias94/merman/blob/v0.11.0/platforms/node/packages/node-wasm/README.md). The native
loader does not switch transports on failure; Node-WASM is separate from the browser package.

Flutter `0.10.0` requires Flutter `3.47.7+` or native Dart `3.10+`. Its thin facade verifies
and downloads only the target library at build time, and the application bundles it for runtime.
For offline builds, prepare the exact target assets or supply a local native directory; a cold
cache cannot download offline. Mirrors retain manifest checksum verification. See the
[Flutter installation and offline options](../../platforms/flutter/README.md#install).
Use the browser SDK when deploying to the web.

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

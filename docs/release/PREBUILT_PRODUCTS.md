# Install Merman for Your Host

Choose the host where Merman will run, then the output your application needs. Cargo features
are for source builds; installing a prebuilt renderer does not require choosing them.

Choose stable **0.9.3** or **0.10.0-alpha.1**, then follow your host's guide. Keep the wrapper
and native/WASM library on the same release. Supported prebuilt installs need no Rust compiler.
The alpha adds SVG math to the ordinary native bindings and changes Options JSON to schema 3;
review the [upgrade actions](../../CHANGELOG.md#breaking-changes-and-upgrade-actions) when migrating.

## Published Versions

Registry metadata checked on 2026-11-03 shows these versions. Channels release separately;
main's workspace version does not establish a download on every registry.

| Channel | Stable version | Alpha version | Alpha installation |
| --- | --- | --- | --- |
| Rust | `0.9.3` | `0.10.0-alpha.1` | `cargo add merman@0.10.0-alpha.1`; [alpha source feature choices](https://github.com/Latias94/merman/blob/v0.10.0-alpha.1/README.md) |
| CLI / standalone LSP | `0.9.3` | `0.10.0-alpha.1` | [Alpha release installers and archives](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.1) |
| Browser / Worker | `0.9.3` | `0.10.0-alpha.1` | `npm install --save-exact @mermanjs/web@0.10.0-alpha.1` |
| Native Node / Node-WASM | [`0.9.3`](https://github.com/Latias94/merman/blob/v0.9.3/platforms/node/README.md) | `0.10.0-alpha.1` | `npm install --save-exact @mermanjs/node@0.10.0-alpha.1` |
| Python | [`0.9.3`](https://github.com/Latias94/merman/blob/v0.9.3/platforms/python/merman/README.md) | `0.10.0a1` | `python -m pip install --only-binary=:all: merman==0.10.0a1` |
| Flutter / native Dart | [`0.9.3`](https://github.com/Latias94/merman/blob/v0.9.3/platforms/flutter/README.md) | `0.10.0-alpha.1` | `flutter pub add 'merman:0.10.0-alpha.1'` |
| Android | [`0.9.3` AAR](https://github.com/Latias94/merman/blob/v0.9.3/platforms/android/README.md) | `0.10.0-alpha.1` AAR / local Maven ZIP | [Android installation](../../platforms/android/README.md) |
| Apple | `0.9.3` legacy XCFramework | `0.10.0-alpha.1` complete local SwiftPM / XCFramework | [Swift installation](#swift-installation) |
| Native C / WASI SDK | `0.9.3` | `0.10.0-alpha.1` | [Alpha SDK downloads and flavor choices](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.1) |

Web and Node alpha packages use the `alpha` dist-tag; stable `latest` remains `0.9.3` for
existing package names. The two new Node Linux ARM64 native packages have only an alpha release.
Select an exact version when reproducible installs matter. Python uses PEP 440 spelling
`0.10.0a1` for the same native release.

The GitHub alpha Release is a prerelease; stable Latest remains `v0.9.3`. Public SwiftPM Git-URL
installation and Maven Central are separate from the downloadable local SwiftPM and Maven
packages below. No public delivery repository or Central coordinate is advertised.

## Recommended Paths

| Your application | Start here | Outputs and boundaries |
| --- | --- | --- |
| Browser or browser Worker | `@mermanjs/web`: [stable guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/web/README.md), [alpha guide](https://github.com/Latias94/merman/blob/v0.10.0-alpha.1/platforms/web/README.md) | Complete browser SDK; use `web-render` for SVG alone, or analysis/editor/ASCII task packages |
| Node, static-site generation or server rendering | [`@mermanjs/node`](../../platforms/node/README.md) | Platform package selected automatically; `node-wasm` is an explicit portable alternative |
| Python | [`merman` wheel](../../platforms/python/merman/README.md) | pip/uv selects a matching native wheel; no Rust for ordinary supported installs |
| Flutter or native Dart | [`merman` on pub.dev](../../platforms/flutter/README.md) | Alpha downloads and verifies only the build target's library; prepare assets or a warm cache for offline builds |
| Kotlin on Android | [Local Maven repository](../../platforms/android/README.md#resolve-a-candidate-maven-publication) | Alpha ZIP retains AAR metadata and transitive dependencies; a standalone AAR is also available |
| Swift on iOS or macOS | [Complete local SwiftPM package](../../platforms/apple/README.md#add-the-complete-swift-package) | Alpha ZIP includes the facade, resources, manifest and XCFramework; add the extracted package in Xcode |
| Native application with C/C++ interoperability | `sdk-native-full` for your target | SVG, PNG, JPEG, PDF and ASCII; math and ELK; caller-owned font bytes |
| Go, Python or another non-JavaScript sandbox host | `sdk-wasi-full` | SVG and PNG; math and ELK; one WASI reactor with a separate ABI |
| Rust, CLI/LSP, Rustdoc, Typst or editor integration | [Package surfaces](PACKAGE_SURFACES.md) | Source crates and independent task products keep their existing installation routes |

Web full/render and the alpha Swift, Android, Python, Flutter and Node products include SVG
math. Stable `0.9.3` native bindings omit math. These native bindings do not include PNG/JPEG/PDF
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
[`Merman-swiftpm-v0.10.0-alpha.1.zip`](https://github.com/Latias94/merman/releases/download/v0.10.0-alpha.1/Merman-swiftpm-v0.10.0-alpha.1.zip)
and its
[SHA-256 checksum](https://github.com/Latias94/merman/releases/download/v0.10.0-alpha.1/Merman-swiftpm-v0.10.0-alpha.1.zip.checksum).
Verify the checksum, extract the archive and add the resulting directory as a local package in
Xcode. Link the `Merman` product. A Swift package can use:

```swift
.package(path: "../Merman-swiftpm-v0.10.0-alpha.1")
```

The archive includes matching Swift sources, resources, manifest, XCFramework and legal material.
No source checkout or Rust compiler is needed. Do not substitute the separately named
`Merman.xcframework-v0.10.0-alpha.1.zip`: that smaller archive supplies only the binary target.

```swift
import Merman

let client = Merman()
let options = try resourceOptionsJson(profile: .interactive, overrides: [])
let svg = try client.renderSvg(source: "flowchart TD\nA[Hello] --> B[World]", optionsJson: options)
```

The alpha also provides a thin remote package tree for publishers. A public Git-URL installation
requires its own hosted and qualified repository; downloading that tree does not make the source
repository URL a working binary package. See the [Apple guide](../../platforms/apple/README.md).

For stable `0.9.3`, use the [legacy Apple instructions](../../platforms/apple/README.md#install-published-v093):
its XCFramework needs the matching source checkout for the facade and `Package.swift`, and omits
math. Keep all inputs at `v0.9.3`.

## Android Installation

Download the complete
[`merman-android-0.10.0-alpha.1-maven.zip`](https://github.com/Latias94/merman/releases/download/v0.10.0-alpha.1/merman-android-0.10.0-alpha.1-maven.zip)
and its
[SHA-256 sidecar](https://github.com/Latias94/merman/releases/download/v0.10.0-alpha.1/merman-android-0.10.0-alpha.1-maven.zip.sha256).
Verify the checksum and extract it to a local directory. The [Gradle example](../../platforms/android/README.md#resolve-a-candidate-maven-publication)
resolves `io.merman:merman-android:0.10.0-alpha.1` with these properties:

```text
-PmermanRepository=/absolute/path/to/extracted/repository
-PmermanGroup=io.merman
-PmermanVersion=0.10.0-alpha.1
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

Python wheel compatibility includes OS, CPU and libc, not just the Python version. Alpha wheels
cover GNU Linux ARM64/x86-64, macOS ARM64/x86-64 and Windows x86-64. Alpha GNU Linux wheels
use manylinux 2.28 and require glibc `2.28+`. Published `0.9.3` has three wheels, including
`manylinux_2_35_x86_64`, and has no source distribution. An unsupported target does not gain
support merely by installing Rust; see the explicit source-helper route in the Python guide.

Node requires Node.js `22+`; GNU Linux native packages require glibc `2.31+`. Native packages
distinguish glibc from musl. Keep optional dependencies enabled and install on the deployment
target; copying macOS `node_modules` to Linux does not select a Linux binary. Alpha adds GNU and
musl Linux ARM64 coverage; it is not present in published `0.9.3` native packages. For a portable
alternative, explicitly install `@mermanjs/node-wasm@0.10.0-alpha.1`
and follow its [Node-WASM guide](../../platforms/node/packages/node-wasm/README.md). The native
loader does not switch transports on failure; Node-WASM is separate from the browser package.

Alpha Flutter requires Flutter `3.47.7+`; native Dart retains `3.10+`. Its thin facade verifies
and downloads only the target library at build time, and the application bundles it for runtime.
For offline builds, prepare the exact target assets or supply a local native directory; a cold
cache cannot download offline. Mirrors retain manifest checksum verification. See the
[Flutter installation and offline options](../../platforms/flutter/README.md#install).
Stable `0.9.3` continues to bundle its libraries. Native Dart support does not imply Flutter Web
support.

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

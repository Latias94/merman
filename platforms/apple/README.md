# Merman For Apple Platforms

Parse, analyze, lay out, and render Mermaid diagrams from Swift on iOS and macOS without a WebView or JavaScript runtime. The `Merman` SwiftPM product is a direct UniFFI binding packaged as a binary XCFramework plus generated Swift source.

The current source examples use Options JSON schema `4`, facts schema `3`, and Theme v2. For migration from `0.10.0` to the published `0.11.0` release, see the [upgrade index](../../docs/release/README.md#merman-0110).

> **Binding contract:** Current source uses direct UniFFI binding API `8` and ASCII output schema `4`; published Merman `0.10.0` wrappers retain API `7`. Options JSON remains schema `3`. Use the Swift source and XCFramework produced by the same Merman build. UniFFI rejects incompatible contract or API checksum pairs, but it does not compare Merman release versions when the generated interface is unchanged.

## Requirements

- Swift 5.9 or newer
- iOS 14 or newer, or macOS 12 or newer
- Xcode on macOS for local package use
- Python 3, the Rust toolchain in `rust-toolchain.toml` with `rustup`, Cargo, and `lipo` only when building the current XCFramework from source

For Swift 5.9 iOS integration, use Xcode 15.2 or newer. The SwiftPM 5.9 command-line client cannot select iOS slices from an XCFramework for `swift build --triple`; that command-line cross-build path requires SwiftPM 5.10 or newer.

## Add the Complete Swift Package

Download [the complete SwiftPM ZIP](https://github.com/Latias94/merman/releases/download/v0.11.0/Merman-swiftpm-v0.11.0.zip)
and [its SHA-256 checksum](https://github.com/Latias94/merman/releases/download/v0.11.0/Merman-swiftpm-v0.11.0.zip.checksum),
and verify the checksum before extraction. For custom builds, use the matching source revision
with [Build The Local Package](#build-the-local-package).
Extract the archive and add the resulting `Merman-swiftpm-v0.11.0` directory as a
local package in Xcode. A Swift package can declare:

```swift
.package(path: "../Merman-swiftpm-v0.11.0")
```

Link the `Merman` product. The package includes matching generated Swift, resource declarations, XCFramework,
manifest and legal material. No Rust checkout or compiler is needed to consume it.

Publishers can use `Merman-swiftpm-remote-v0.11.0.zip` as the basis for a hosted delivery
repository. The ZIP contains the manifest, generated facade and legal material. Its manifest
pins `Merman.xcframework-v0.11.0.zip` by URL and checksum. Public Git-URL installation requires
hosting and qualifying that repository separately.

<a id="install-published-v093"></a>

## Legacy: Install Published v0.9.3

Download the [0.9.3 archive](https://github.com/Latias94/merman/releases/download/v0.9.3/Merman.xcframework-v0.9.3.zip)
and [checksum](https://github.com/Latias94/merman/releases/download/v0.9.3/Merman.xcframework-v0.9.3.zip.checksum),
and use the matching `v0.9.3` checkout.

Check out the source tree at the same release tag as the archive, then extract `Merman.xcframework-<tag>.zip` from [GitHub Releases](https://github.com/Latias94/merman/releases) so the framework is located at `platforms/apple/Merman.xcframework`. Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product.

The published v0.9.3 archive contains the binary XCFramework and legal material.
Its matching checkout supplies the generated Swift facade and `Package.swift`;
mixing tags is unsupported. That artifact uses Options JSON schema `2`, omits math and
binary codecs, and does not provide the complete SwiftPM archives. Follow its
[tagged guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/apple/README.md) for API
examples. The Theme v2 examples in this README require artifacts built from the current source.

## Build The Local Package

Build the source, Swift facade and XCFramework together for the selected version.

```sh
bash scripts/build-apple-xcframework.sh
```

Add the repository root as a local package in Xcode or SwiftPM and link the `Merman` product.
The root `Package.swift` resolves `platforms/apple/Merman.xcframework` by path and remains
the source-development manifest. After a complete build, produce a self-contained
distribution from a clean source tree:

```text
python3 scripts/apple_package.py --output-dir target/apple-package --version 0.11.0
```

Partial `--ios` or `--macos` builds remain useful locally but cannot be packaged as
the complete distribution. Source builds also require
`rustup component add llvm-tools-preview` for distribution-copy cleanup.

## Prepare The Remote SwiftPM Handoff

From the clean, matching source revision and its complete staged XCFramework, select the
intended HTTPS upload destination for the binary ZIP:

```text
python3 scripts/apple_package.py --output-dir target/apple-package --version 0.11.0 \
  --source-commit <source-commit> \
  --artifact-url https://github.com/Latias94/merman/releases/download/v0.11.0/Merman.xcframework-v0.11.0.zip
```

The producer freezes these seven release assets:

| Asset | Use |
| --- | --- |
| `Merman.xcframework-v0.11.0.zip` and `.zip.checksum` | Binary-target input and its SHA-256 |
| `Merman-swiftpm-v0.11.0.zip` and `.zip.checksum` | Complete local Swift package and its SHA-256 |
| `Merman-swiftpm-remote-v0.11.0.zip` and `.zip.checksum` | Complete thin delivery repository tree and its SHA-256 |
| `Merman-swiftpm-v0.11.0.Package.swift` | Identical copy of the thin tree's remote manifest |

The extracted remote directory contains `Package.swift`, `Sources/Merman`, `README.md`,
`SOURCE_AVAILABILITY.txt`, `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES`.
Its facade, resource contract and legal files come from the same staging tree as the
local package. Only the installation README and binary reference differ. The remote
manifest binds the separate **XCFramework ZIP checksum**, never either Swift package
archive checksum. There is no extra artifact identity or fingerprint format.

After separate publication authorization, commit the **contents** of the extracted
remote directory at a delivery repository's root. Publish the exact binary ZIP at the
manifest's HTTPS URL, qualify the repository commit below, then tag that delivery
commit for SwiftPM. Do not regenerate bindings, edit the manifest or rebuild native
libraries in that repository. Its Git commit can differ from the Merman source commit
recorded in `SOURCE_AVAILABILITY.txt`; that separation avoids a source/checksum cycle.
Neither the packager nor the release workflow creates or pushes a delivery repository.

## Qualify A Remote Dependency

Use the final thin archive as the expected source tree, an existing HTTPS Git
repository containing its unmodified contents, and an exact delivery commit:

```text
python3 scripts/apple_qualify.py \
  --package-archive target/apple-package/Merman-swiftpm-remote-v0.11.0.zip \
  --work-dir /absolute/fresh/apple-remote-proof --version 0.11.0 --simulator \
  --remote-package-url https://your-controlled-host/Merman.git \
  --remote-package-revision <40-character-delivery-commit>
```

Each consumer uses `.package(url:revision:)`, a fresh project and its own SwiftPM
cache. SwiftPM's `dump-package` first confirms a version-matched HTTPS binary target.
SwiftPM resolves the Git dependency and downloads/checksums that binary;
the qualifier compares the resolved commit, tracked file inventory and file bytes
with the thin archive before building. It never substitutes a local package on
failure. macOS executes the catalog, ordinary SVG/ELK, math and policy/codec controls;
iOS device is compile/link checked, and `--simulator` also executes those controls on
the host architecture's simulator. The exact runtime version must match `--version`.
`qualification.json` is written only after all requested checks pass. A local-package
run or Python unit test is not remote transport evidence.

The release owner's optional `remote_package_url` and `remote_package_revision`
dispatch inputs run this same check against the build's thin archive on macOS. With
`publish: true`, that job waits for the explicit seven-asset upload and public readback;
with `publish: false`, the exact binary must already be reachable at its manifest URL.
Omitting both inputs skips remote qualification and supplies no HTTPS proof. Local
qualification and the existing bitcode-cleaned/original comparison remain separate.

### Controlled HTTPS Acceptance Without A Public Repository

Release preflight's `apple-xcframework` job and the Apple release owner's `build`
job always run this controlled fixture after their existing build and local checks,
even when the optional external-repository inputs above are empty. Those inputs
control qualification of an independently hosted delivery repository; the controlled
fixture supplies localhost HTTPS transport evidence on every successful candidate.

From a clean, selected source revision with its complete, freshly built XCFramework,
use the candidate version, exact source commit and a fresh work directory:

```text
python3 platforms/apple/tests/qualify_remote.py \
  --work-dir target/apple-https-qualification --version 0.11.0 \
  --source-commit <40-character-source-commit> \
  --xcframework platforms/apple/Merman.xcframework
```

The bounded helper uses Python's standard library and the existing `package_apple`
and `apple_qualify` operators. It packages the same staged XCFramework with an HTTPS
binary URL using the port actually reserved by its localhost TLS server. It prefers
8443 and falls back to an OS-assigned port when 8443 is occupied. The thin tree is
committed unchanged into a temporary Git repository, cloned bare and served using
`git update-server-info` and Git's standard dumb HTTP transport. No public repository
is created or pushed, and Rust is never invoked by the helper. The existing clean
source, matching facade, complete slice and production HTTPS validators still apply.

Use a disposable macOS VM or CI runner with Xcode, an installed iOS simulator
and noninteractive `sudo -n` access, as provided by the hosted macOS runner.
The helper generates its own one-day localhost certificate and adds administrator-domain
trust with `sudo -n security add-trusted-cert -d -r trustRoot` in
`/Library/Keychains/System.keychain`. User-domain trust requires an authentication dialog
and is unsuitable for this unattended fixture. Git uses the fixture certificate through
`GIT_SSL_CAINFO`; SwiftPM's binary downloader uses the macOS trust entry.
The `finally` cleanup replaces this certificate's administrator trust with explicit `deny`.
It deletes only that certificate's Keychain entry, selected by its fingerprint. It then
supplies the certificate itself to a fresh `security verify-cert` process, without a
custom trust anchor, and requires `kSecTrustResultDeny`, and checks that the Keychain entry
is absent.
Removing the last administrator trust record can require an interactive authorization
dialog on macOS 15; changing the authorization database is also denied there. The helper
uses neither operation. Its explicit deny record persists until the disposable runner is
destroyed, so do not run this fixture on a persistent development account. Each security
command has a 30-second timeout; failed revocation or verification still attempts certificate
deletion and prevents a successful receipt. The helper leaves other certificate settings,
Keychain defaults and search lists unchanged. Its temporary TLS key, served ZIPs and Git
repositories are removed.
No HTTP dependency URLs, TLS verification bypasses, Git URL rewriting or SwiftPM
mirrors are used.

The positive run delegates exact `.package(url:revision:)` resolution, binary-target
HTTPS download and checksum validation, strict source comparison, macOS execution,
iOS compile/link checks and Simulator execution to `apple_qualify`. Only after that
run succeeds does the helper corrupt its disposable served XCFramework ZIP. A second
run uses fresh consumer/cache directories, the same manifest and exact Git revision.
It must fail with SwiftPM's actual `MermanFFI` downloaded-artifact checksum mismatch
diagnostic and must not write a successful qualification receipt. An unrelated TLS,
Git or compiler error cannot satisfy the negative check. Release staging stays frozen.

The work directory retains `proof/qualification.json`, `proof/installed/*.json`,
resolved consumer `Package.resolved` files, `proof.log`, `https-requests.log` and
`wrong-checksum.log`. Its top-level `qualification.json` is written only after both
runs and certificate cleanup succeed. Both workflows retain these files as
qualification artifacts, including available logs on failure. Keep this evidence in
ignored `target/` or outside the source checkout. This localhost proof does not
establish public GitHub download availability; the release owner separately compares
every published asset byte for byte and checks all three archive checksums without
authentication.

## Render A Diagram

```swift
import Merman

let client = Merman()
let source = "flowchart TD\nA[Hello] --> B[World]"
let options = try resourceOptionsJson(profile: .constrained, overrides: [])
let svg = try client.renderSvg(source: source, optionsJson: options)
precondition(svg.hasPrefix("<svg"))
```

`resourceOptionsJson` emits Options JSON schema `4`. Pass `nil` as the profile for a reusable request overlay that must inherit its constructor ceiling; generated override records accept only `MermanResourceOverrideId` values.

Use `MermanOperationRequestV4` and `client.execute(request:)` when the selected output is dynamic; put its options in the request's `optionsJson` field and pass `control: nil` when cancellation is not needed. The generated `MermanOperationResult` carries binary-safe bytes, media type, and typed operation metadata. For repeated work, construct `try MermanEngine(optionsJson:services:)` directly with baseline options and an optional immutable `MermanEngineServices` bundle. Per-operation options deep-merge over that baseline but cannot change the constructor-owned runtime policy. Call `close()` deterministically when an engine may retain foreign services; close is idempotent and retryable after busy or reentrant failures.

The current-source binding API is 8. `MermanOperationRequestV4` remains the request record name;
it does not identify the binding API version. Replace `bindingApiVersionV7()` with
`bindingApiVersionV8()` and regenerate Swift source and the XCFramework together. ASCII output
schema `4` adds `requestedGraphPresentation` and replaces `overflowed` with nullable
`primaryOverflowed` and required `emittedOverflowed`. `primaryWidth`, `primaryHeight`,
`layoutProfile`, and `primaryOverflowed` are all present or all `nil` when no valid primary layout
was produced. Preserve measured `0` and `false`; use emitted dimensions and overflow to describe
the actual output without filling missing primary fields. `requestedLayoutProfile` and
`compactAttempted` retain native selection evidence. Discover policies through capability
`graphPresentations` and request `ascii.graph_presentation` (`strict` by default, or `adaptive`
for admitted families); the native renderer owns layout and fallback selection. Options JSON
schema `3`, outer operation metadata schema `1`, and C ABI `3` are unchanged.
`MermanError.Binding` includes an optional
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

The default XCFramework includes semantic and layout JSON, analysis, validation, document
analysis, ASCII, SVG, math, Cytoscape and ELK. PNG, JPEG and PDF remain typed absences.
For formulas, use `resourceOptionsJson(profile: .interactive, overrides: [])` when
constructing the engine; constrained policy still rejects opaque math. Compiled math
does not remove the need for host execution/memory limits. Swift's host measurement
callback is independent of the native C/WASI memory-font constructor, which this
package does not expose. Check `runtimeCatalogJson()` for the installed artifact's
actual capability IDs and `themeCatalogJson()` for presentation choices. Catalog IDs
remain open strings.

## Diagram Themes

Decode `client.themeCatalogJson()` to discover the schema-2 catalog with 13 built-in presets, classic Mermaid IDs, and native theme capability flags by family. Each preset lists its supported appearances, default appearance, and six swatches for the default.

```swift
let request = MermanOperationRequestV4(
    operationId: "export-theme-preset", source: "studio", uri: nil,
    optionsJson: nil, control: nil
)
let exported = try client.execute(request: request)
let themeJson = String(decoding: exported.data, as: UTF8.self)
let input = try JSONSerialization.data(withJSONObject: ["document": themeJson, "appearance": "dark"])
let resolution = try client.execute(request: MermanOperationRequestV4(
    operationId: "resolve-theme", source: String(decoding: input, as: UTF8.self),
    uri: nil, optionsJson: nil, control: nil
))
let checked = try JSONSerialization.jsonObject(with: resolution.data) as? [String: Any]
let document = try JSONSerialization.jsonObject(with: exported.data)
let options = try JSONSerialization.data(withJSONObject: [
    "theme": ["document": document, "appearance": "dark"],
    "background": "transparent",
] as [String: Any])
let rendered = try client.execute(request: MermanOperationRequestV4(
    operationId: "svg", source: "flowchart LR\nA[Review] --> B[Publish]", uri: nil,
    optionsJson: String(decoding: options, as: UTF8.self),
    control: nil
))
let metadata = try JSONSerialization.jsonObject(with: Data(rendered.metadata.rawJson.utf8))
```

`export-theme-preset` takes the raw preset ID and returns editable Theme v2 JSON. In `resolve-theme`, pass the document as an object or as original JSON text. Successful responses contain `resolved` and `notices`. Document errors carry `line`, `column`, `path`, and `message`. Set `auto_fix_contrast` in the request envelope to enable contrast correction. SVG results describe the actual theme in `metadata.rawJson` under `theme_outcome`. [Authoring smoke](examples/smoke/Sources/MermanAppleSmoke/ThemeAuthoring.swift)

For rendering, `theme` accepts a preset ID, an inline Theme v2 document, or a selector with `preset` or `document` and optional `appearance` and `precedence`. For a reusable engine, omit `theme` to inherit the selection or pass `"none"` to clear it for one request. Choose classic Mermaid themes with `site_config.theme`. Use `background` to replace the canvas with a color or `"transparent"`. On custom artifacts with PNG, JPEG, or PDF export, use `export_matte` for an opaque export backing color. [Theme v2 schema](../../schemas/theme-v2.schema.json)

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

The macOS smoke checks the theme catalog, exports Studio, resolves both object and text forms of the document, and checks document-relative diagnostics. It also verifies the selected appearance in SVG metadata and the visible canvas override through both one-shot and reusable clients.

## Documentation And Releases

- [Apple binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [UniFFI binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) beside the Swift package or XCFramework.

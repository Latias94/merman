# Binding delivery and installation study

Date: 2026-11-01. Source inspected: main commit
`939ccec9f8d1391ae672a5397d5b57d10a1307b8`, after PR #265. This is a recommendation
for future main-line work, not a release declaration or a change to v0.9.x.

## Recommendation

Keep the existing language transports and publish through each ecosystem's normal dependency
manager. Give each host one clear recommended rendering installation, retain task-specific
packages where they avoid a substantial dependency, and move reduced feature selection into
advanced documentation. Complete the delivery of matching wrappers and binaries rather than
asking users to assemble a binding from a Rust checkout.

The most valuable next steps are remote SwiftPM installation, repository-based Android
installation, broader Python wheel coverage, and a sustainable Flutter native-asset route.
Node's platform-package structure and the native C/WASI SDK packaging are foundations to keep.
Browser downloads need their own size and initialization decisions. There is no reason to
replace all transports with C ABI, migrate Python to PyO3, or impose the native SDK's three
flavors on every language.

The recommendations below are our architectural conclusions from the repository and the
first-party sources cited. A comparison project's packaging is evidence of a useful pattern,
not proof that its exact product or compatibility policy fits Merman.

## Observed delivery, separate from source capabilities

Official registry metadata was read on **2026-11-01 at 03:50 UTC**. These observations prove
version visibility, not that every published artifact was downloaded and executed in this
study.

| Channel | Observed public version | Evidence |
| --- | --- | --- |
| Rust facade, CLI and LSP | `0.9.3` | crates.io APIs for [merman](https://crates.io/api/v1/crates/merman), [merman-cli](https://crates.io/api/v1/crates/merman-cli), and [merman-lsp](https://crates.io/api/v1/crates/merman-lsp) |
| Five browser npm packages | `latest: 0.9.2`; `alpha: 0.8.0-alpha.7` | npm package-group dist-tags, including [web](https://registry.npmjs.org/-/package/%40mermanjs%2Fweb/dist-tags) |
| Node loader, five platform packages, explicit Node WASM | `latest: 0.9.2`; `alpha: 0.8.0-alpha.7` | npm package-group dist-tags, including [node](https://registry.npmjs.org/-/package/%40mermanjs%2Fnode/dist-tags) |
| Python | `0.9.2`, three wheels, no sdist | [PyPI release metadata](https://pypi.org/pypi/merman/json) |
| Flutter/Dart | `0.9.2` | [pub.dev package API](https://pub.dev/api/packages/merman) |
| Tree-sitter grammar | `0.3.1` on npm and crates.io | [npm dist-tags](https://registry.npmjs.org/-/package/%40mermanjs%2Ftree-sitter-mermaid/dist-tags), [crates.io API](https://crates.io/api/v1/crates/tree-sitter-mermaid) |

The repository's [package surface record](../release/PACKAGE_SURFACES.md) also records native
C/WASI SDK and legacy Apple XCFramework delivery at `0.9.3`, Android AAR delivery at `0.9.2`,
and independently versioned Typst and VS Code products. Historical binary downloads were not
repeated here. Main's version string is still `0.9.3`; its Options schema 3, Mermaid 12.2,
and new Apple math-enabled complete package are unreleased source capabilities. They cannot
be attached to immutable maintenance `v0.9.3` as if that were the same source.

PR #265 already completed the self-contained local SwiftPM package, binary ZIP/checksum
handoff, and distribution-copy cleanup. Remote URL-based SwiftPM resolution remains a
separate delivery step. Do not reopen that implementation or describe it as already hosted.

Different channels may ship at different times. The public install page and Release body
should show each channel's actual version and capabilities; workspace metadata and green
preflight are insufficient substitutes.

## Make the first choice about the application

The first page should ask **where will Merman run?** and, for rendering, **which output is
needed?** It should return an ordinary installation command and a small capability summary.
Cargo feature names, internal recipe IDs, ABI revisions, and producer target matrices belong
behind the advanced link unless a consumer actually needs them.

Keep three decisions distinct:

1. **Document semantics:** diagram families, formula labels, layout algorithms, themes, and
   any explicitly scoped semantic limitations.
2. **Host services and outputs:** SVG, ASCII, image/PDF encoders, font bytes or measurement
   callbacks, filesystem/system-font access, threading, and runtime integration.
3. **Delivery compatibility:** operating-system floor, architecture, libc, language runtime,
   wrapper/native version pairing, and installation size.

For native prebuilt rendering products, recommend a complete ordinary Mermaid SVG experience,
including math and ELK, subject to actual host qualification and artifact budgets. Apple main
already does this; Android, Python, Flutter and Node do not yet all do so. This is a proposed
product change, not a statement about installed `0.9.2` packages. Extra encoders and ambient
system services should be added only where the host API and use case justify them.

For browsers, keep renderer, analysis, editor and ASCII task packages separate. Recommend
math in the existing full and render products for the next feature release, after measuring
the actual incremental compressed bytes, initialization, memory, and Worker behavior and
making an explicit budget decision. Published `0.9.2` full/render recipes already include
math; main's recipes currently omit it. That difference matters: preserving the published
complete-SVG expectation is preferable to quietly treating removal as a size optimization.
See the [immutable release recipes](https://github.com/Latias94/merman/blob/v0.9.2/capabilities/artifact-profiles-v2.json).
No extra npm math tier is proposed. The main-line recommendation would deliberately amend
the Web/Node part of [ADR-0097](../adr/0097-explicit-math-capability.md), while preserving
source math opt-in. It is not already authorized by the Apple/C/WASI pilot.

Source consumers retain additive Cargo features and can deliberately omit expensive
capabilities. [Cargo's feature guidance](https://doc.rust-lang.org/cargo/reference/features.html)
supports additive selection; prebuilt convenience does not require changing Rust defaults.
Keep the existing descriptor and loaded runtime catalog as their respective authorities.
Do not add a second feature matrix inferred from package names or another fingerprint layer.

## Recommended installation by binding

These are target recommendations. Any route missing today is identified explicitly.

| Consumer | Primary route | Product recommendation | Current gap / useful fallback |
| --- | --- | --- | --- |
| Rust | crates.io source dependencies | `merman` for rendering, `merman-core` for parsing, `merman-analysis` for analysis; explicit Cargo feature selection | Keep source delivery and documented features; do not publish compiler-specific prebuilt `rlib` files as the ordinary install. |
| Browser / Worker | Existing `@mermanjs/web*` npm packages | Lead with `@mermanjs/web`; offer render, analysis, ASCII and editor for focused tasks | Publish portable WASM with its JS/types and documented bundler/self-host loading. Preserve complete SVG math in full/render subject to measured web budgets. |
| Node / server-side rendering | `@mermanjs/node` | Small loader plus exact-version native optional dependencies; explicit `@mermanjs/node-wasm` alternative | Add qualified Linux ARM64 coverage; keep native and WASM choices explicit and make unsupported-target errors actionable. |
| Python | `pip install merman` | One matching generated facade/native wheel per platform, independent of CPython minor ABI when valid | Broaden wheel coverage and lower the measured glibc floor. Retain an explicit source-build route until a real tested sdist exists. |
| Swift / iOS / macOS | Tagged remote SwiftPM package | Matching Swift sources plus checksum-bound remote XCFramework; ordinary SVG with math and ELK | Complete hosted SwiftPM handoff. Keep the already implemented complete local package for offline/manual use. |
| Kotlin / Android | Maven Central coordinates | One AAR with matching Kotlin/JNI, release metadata, and supported ABI slices | Remote namespace/publication is not established. Ship a ZIP of the complete verified Maven staging as an interim/offline route. |
| Flutter / native Dart | `flutter pub add merman` / `dart pub add merman` | One Dart API using native-asset hooks to acquire only the selected target | Move beyond placing every native platform in the pub archive; preserve pinned verification, offline use and explicit local/source overrides. |
| C / C++ | Existing target-specific full SDK archive | Headers, static/shared libraries, CMake imported targets, pkg-config, examples, fonts and notices | Improve compatibility declarations; add vcpkg or Conan only when concrete consumers justify another owner. Reduced SDK recipes remain advanced. |
| WASI / Go / sandbox hosts | Existing `sdk-wasi-full` archive and real host examples | One supported non-JS reactor with its own ABI; caller-owned fonts and host policies | Document supported wazero/wasmtime host setup. Do not present examples as published Go or Python language packages. |
| Typst | Versioned `@preview/merman` package | Idiomatic Typst wrapper containing its matching plugin WASM | Keep independent wrapper version and bundled engine mapping; evaluate formula support against Typst's actual plugin/runtime/size constraints. |
| CLI / standalone LSP | Existing cargo-dist archives, installers, cargo-binstall and ecosystem packages | Complete CLI; separate LSP executable | Preserve established owners and minimum-system checks. A source `cargo install` may select different optional capabilities. |
| VS Code | Platform-specific Marketplace installation, when selected for release | Extension plus its matching CLI/LSP binaries, with no Rust setup | Currently only verified VSIX artifacts are produced. Preserve direct VSIX as the offline route; Marketplace is future work. |
| Tree-sitter grammar | Existing npm and crates.io grammar packages | Grammar, queries, language WASM and native binding with matching grammar versions | Preserve its independent version and generic runtime boundary. Syntax-only users should not download the renderer. |

### Swift and Android: finish the package-manager handoff

Use a small SwiftPM delivery repository whose root contains the generated remote manifest,
matching Swift sources and notices. Its binary target points to the exact separately published
XCFramework ZIP and checksum. Keep native source identity and delivery repository identity
distinct: first freeze/build native source, then form the delivery commit from those bytes.
This avoids trying to embed a ZIP digest in the very source commit whose digest-dependent
build produces that ZIP. The full local ZIP remains a supported and useful second route.

[Apple's binary package design](https://developer.apple.com/videos/play/wwdc2020/10147/),
[Matrix's Swift package](https://github.com/matrix-org/matrix-rust-components-swift/blob/main/Package.swift),
and [Loro's Swift package](https://github.com/loro-dev/loro-swift/blob/main/Package.swift)
provide direct examples. A separate delivery repository is our recommendation, not an Apple
requirement. Mozilla's old independent Swift repository is deprecated; its
[current design](https://mozilla.github.io/application-services/book/design/swift-package-manager.html)
uses a local consumer package around a remote binary. Do not cite the deprecated design as
the only current practice.

For Android, reuse `maven-publish`, the existing `localStaging` publication and its verifier.
Retain the AAR, POM, Gradle Module Metadata, sources and documentation JARs, checksums and
required signing material. Select a verified Central namespace before documenting real
coordinates; `io.merman` in a build file does not establish namespace ownership. Maven group
selection need not rename the Kotlin API package.

[Android explicitly recommends repository distribution](https://developer.android.com/build/publish-library/upload-library)
and supports a ZIP of a folder-based Maven repository. This makes the complete staging ZIP
a useful interim product, ahead of Central promotion. A bare AAR remains a manual fallback,
with its dependency metadata limitations. Use a standard Portal integration if needed;
Gradle's publishing plugin alone is not a Central Portal uploader. Preserve qualified bytes
through validation and promotion rather than rebuilding after approval.

For Android consumers, verify the actual native ELF alignment, final APK alignment and
execution on a 16 KB environment. New AGP/NDK versions alone do not prove a Rust-produced
library or consumer APK meets [Android's page-size requirements](https://developer.android.com/guide/practices/page-sizes).

### Python: fix installation reach before replacing the binding

PyPI currently exposes only `py3-none-macosx_11_0_arm64`,
`py3-none-manylinux_2_35_x86_64`, and `py3-none-win_amd64` wheels. Their compressed sizes
are approximately 7.6, 8.4 and 8.3 MB respectively. Intel macOS and Linux ARM64 are
missing; Alpine/musl is a separate compatibility decision. A glibc 2.35 floor also excludes
older otherwise common Linux installations.

The generated UniFFI/ctypes wrapper is not a CPython extension. Its `py3-none-<platform>`
wheel tag is not inherently wrong, and converting it to PyO3 would add a binding migration
without directly solving those coverage gaps. Recommend wheel coverage and honest minimum
runtime metadata first. Build in a selected manylinux baseline, inspect/repair the actual
shared-library dependencies, then execute the installed wheel there. Do not lower the wheel
tag without lowering and qualifying the actual binary requirements.

Use `manylinux_2_28` as an initial floor candidate to investigate, not an already qualified
promise. Add GNU Linux ARM64 and Intel macOS next; evaluate `musllinux_1_2` separately when
Alpine users justify it. The [manylinux specification](https://peps.python.org/pep-0600/)
and [auditwheel's limitations](https://github.com/pypa/auditwheel#limitations) make the
important distinction: repairing/bundling eligible libraries cannot remove a too-new glibc
symbol requirement.

[Maturin documents UniFFI as its own supported binding mode](https://www.maturin.rs/bindings.html#uniffi),
so a later move to that packaging tool would not inherently require PyO3. Keep the installed
wrapper's declared Python minimum and qualify the interpreters actually supported; a generic
wheel ABI tag alone does not prove PyPy or free-threaded runtime compatibility.

Keep normal `pip` installation free of Rust and manual native-library lookup. A source
archive should be published only if it really builds the matching Rust/native package with
documented prerequisites; the current PyPI release has no sdist. Choosing a packaging tool
that supports UniFFI is independent of choosing a different Python FFI architecture.

[cryptography's installation guide](https://cryptography.io/en/latest/installation/) is a
useful user-facing example: ordinary pip/uv installation uses wheels, while compiler and
native dependencies are explained on the deliberate source-build path. Adopt that clear
separation without copying its OpenSSL-specific build or claiming its platform matrix.

### Flutter: use native assets without a universal download

Keep one public Dart package and the existing hooks/code-assets integration. A target-aware
hook can fetch the exact native asset for the requested OS, architecture and Apple SDK,
verify it against package-version-pinned digests, and reuse the build cache. It should also
accept a local verified artifact for offline/enterprise builds and an explicit source-build
choice for unsupported targets or custom features.

This is a future packaging change: the current hook selects a file already inside the
package. Changing it to download assets moves a network dependency from pub resolution to
the native build, so offline preparation and clear download failure messages are part of
the design. Native code assets do not establish Flutter Web support. Keep the existing
host Dart/FFI smokes, add packaged application execution, and add registry readback after
an actual new publication. The current host smoke does not prove Android/iOS app bundling.

[Dart's hook documentation](https://dart.dev/tools/hooks) explicitly supports downloading or
compiling native assets. [sqlite3.dart's hook design](https://github.com/simolus3/sqlite3.dart/blob/main/sqlite3/doc/hook.md)
is a close maintained example: version-pinned prebuilt release assets, digest verification
and configurable native-library sources. Reuse the ecosystem hook/cache/configuration model;
do not add an application-runtime downloader or an unrelated plugin for each target.

### JavaScript: retain deliberate runtime and task boundaries

For Node, the present loader plus exact-version OS/CPU/libc packages is a good structure.
Native installation should remain free of a compiler and surprise runtime downloads.
`@mermanjs/node-wasm` stays an explicit alternative, rather than masking every native load
failure with a different runtime. Distinguish missing optional packages, unsupported target,
incompatible runtime and genuine native load failure in the guidance.

[resvg-js](https://github.com/thx/resvg-js/blob/main/README.md) is a directly relevant Rust
rendering example with prebuilt native Node bindings and a separate WASM product, without
requiring node-gyp or postinstall compilation for its normal supported installation.

For browsers, keep actual WASM artifacts in the matching npm product, explicit initialization,
custom URL/loader support and self-hosting. Show one ordinary bundler example and one Worker
example. SSR documentation should point to the Node products where appropriate rather than
requiring a fake browser environment. Avoid adding another general-purpose JS package just
to shorten the selection page.

The existing root `MERMAN_WASM_URL` and custom loader are useful contracts. A narrowly
exported raw WASM asset can improve bundler integration later, without exposing generated
glue or reviving removed capability subpaths. Qualify each browser package installed alone,
at a non-root base URL, in a module Worker and with an appropriate WASM CSP. For Node,
qualify root-only installation so explicitly adding a platform tarball cannot hide optional
dependency resolution failures. Also execute the declared Node minimum, not only the newer
runtime used to build the release.

[CanvasKit's package documentation](https://skia.googlesource.com/skia/+/refs/heads/main/modules/canvaskit/npm_build/README.md)
shows explicit asset location and several task-oriented bundles. It does not establish that
Merman should combine all WASM products into one tarball: subpath selection changes what
is loaded by a page, not how much an npm install downloads.

### Native SDKs and WASI: preserve the usable archive

The C SDK already has a useful complete package: matching headers and libraries,
`find_package(Merman CONFIG REQUIRED)`, `Merman::Merman` / `Merman::Static`, pkg-config,
installed consumers, fonts and notices. This is a valid C/C++ delivery route. Projects such
as [ONNX Runtime](https://onnxruntime.ai/docs/get-started/with-c.html) also provide
language-neutral prebuilt archives alongside package-manager entry points. This does not
require Merman to adopt ONNX Runtime's product variants or publish every C registry.

The GNU C SDK currently qualifies on Ubuntu 24.04. That is not evidence of older glibc or
musl compatibility. Select and measure the intended baseline separately from CLI/Linux
compatibility, which already has its own owner and checks. Record macOS minimum versions
and Windows runtime requirements from the delivered binaries, including static-link
system dependencies.

Use the full WASI SDK for ordinary non-JavaScript sandbox rendering. Keep its own scalar
ABI and supported host integration; it is not the browser wasm-bindgen module, native C
function table, or Typst plugin. The existing Go/wazero and Python/wasmtime examples are
valuable executable documentation. A small Go wrapper can follow demonstrated demand, with
caller-provided or explicitly embedded module bytes, without hidden network fetching or a
new renderer implementation. [wazero's host model](https://wazero.io/docs/) fits this route.

Keep the current reviewed WASI import policy and real-runtime qualification. Do not promise
arbitrary WASI host compatibility from the target triple alone, or migrate to a new
component ABI solely to make a release matrix look uniform.

### Typst, Rustdoc, grammar and editor products: package the actual task

Typst should continue to ship an idiomatic wrapper containing its matching WASM, just as
the [official plugin documentation](https://typst.app/docs/reference/foundations/plugin)
describes. Preserve independent wrapper versions and the bundled engine/ABI mapping.
Typst's deterministic measurement and plugin constraints remain explicit. Its lack of math
cannot be repaired by substituting the WASI module; formula delivery is separate product
work with actual Typst compilation and size evidence.

Rustdoc's generated-fragment CLI route and its proc-macro route serve different users.
Keep the former free of a consumer renderer dependency and the latter as source Cargo
integration. Preserve the grammar's independent matching npm/Cargo release and generic
Tree-sitter runtime. A syntax-highlighting dependency should not pull in semantic rendering.

For VS Code, the current platform-specific VSIX already pairs CLI and LSP without Rust.
If Marketplace delivery is selected, promote those tested packages and verify the installed
extension, rather than adding an activation-time installer. Keep an offline VSIX route.
The [current VS Code publishing guide](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
documents platform packages and federated publishing; new automation should follow that
current guidance rather than copying this repository's older PAT-only prerequisite text.
Standalone CLI/LSP delivery should keep cargo-dist and its existing installer and
package-manager owners. Another universal installer would create work without simplifying
the ordinary language-native routes.

[cargo-dist](https://axodotdev.github.io/cargo-dist/book/) already separates building from
hosting, publication and announcement. Retain its established CLI/LSP route rather than
extending native-library packaging scripts into another executable distribution system.

## Release and user-visible capability rules

1. **Freeze each coupled package group.** Generated wrappers, native/WASM payloads, notices
   and types come from the selected source and version. Keep ABI, options schema, wrapper
   API and product version as distinct identities.
2. **Publish dependencies before the entry point.** Node platform packages precede the
   loader; a remote Swift binary URL/checksum must work before its delivery tag. Existing
   group owners and promotion receipts should enforce this, not a new cross-registry engine.
3. **Use prerelease channels deliberately.** A new alpha must be on the intended npm tag
   for every group member. Today's older `alpha` tags do not represent main readiness.
   Keep stable `latest` unchanged until stable publication is selected.
4. **Prove a fresh consumer of the final package.** Resolve from the intended registry or
   public URL without checkout dependencies, import, inspect the actual catalog, and render
   representative ordinary, ELK and formula inputs where promised. Use the same bounded
   behavior assertions across local package qualification and public-package checks.
5. **Show actual availability.** Release notes list delivered channels and the verified
   versions of intentionally deferred channels. A candidate build, uploaded workflow artifact,
   source crate, bare AAR or generated manifest does not establish a registry install route.
6. **Keep absence actionable.** Explain missing math, encoder, font service or layout from
   the loaded artifact. Never turn an unsupported formula into an empty successful label.
   Existing typed errors and runtime catalogs are the correct foundations.

The install page needs only the package, actual version, supported host range, formula/ELK
availability, outputs, font input/measurement route and links to advanced choices. Keep
checksums, source identities and producer receipts in existing operator records; normal
users should not have to interpret a provenance graph to render their first diagram.

Two operator prerequisites need refreshing before a future implementation: npm now documents
[OIDC dist-tag management with separate permission](https://docs.npmjs.com/trusted-publishers/#managing-dist-tags-with-trusted-publishing),
and VS Code documents federated publication. Existing repository prose claiming that npm
Trusted Publishing inherently cannot change tags, or treating a long-lived VS Code PAT as
the only option, is no longer a sufficient design basis. This study did not inspect those
accounts' actual permissions, so it establishes no new publishing authority.

Math capability and resource policy remain separate. Formula quickstarts must select the
existing supported policy, for example the interactive helper in the Apple guide. Installing
a formula-capable binary does not imply that explicitly constrained execution admits its
opaque dependency. Font coverage is also separate: bundled Cousine is a limited fallback,
math uses its own font set, and text-preserving SVG still depends on viewer fonts. Do not
equate additional encoders with complete Unicode or portable font rendering.

Measure size at the consumer's boundary: compressed registry/download bytes, selected-target
native bytes, and linked/installed application contribution are different quantities. A
multi-architecture XCFramework archive is not an iPhone application's final size. Browser
transfer and initialization, Python's selected wheel, and Flutter's selected native target
also need their own measurements. Preserve existing budgets until a measured product
decision justifies changing them.

## Proposed sequence and acceptance

| Priority | Work | Completion evidence |
| --- | --- | --- |
| P0 | One host-oriented installation page with per-channel actual availability and simple capability explanations | Every recommended command points to a real supported route; future routes are visibly marked. Reconcile selected channels after publication. |
| P0 | Complete remote SwiftPM delivery; retain the complete local ZIP | Fresh consumer resolves the tagged public URL, uses matching Swift/native bytes, executes on macOS and simulator, and compiles device integration without Rust. |
| P0 | Expose Android's complete Maven staging, then Central promotion | External Gradle app resolves the coordinate/metadata, packages JNI, and runs representative rendering. Inspect both ABIs and prove 16 KB behavior before claiming it. |
| P1 | Broader Python wheel matrix and a deliberate Linux floor | Fresh `pip` installs and renders without Rust on each declared target/baseline. Unsupported targets get explicit guidance; no false lower wheel tags. |
| P1 | Establish complete native SVG capability policy for Android/Python/Flutter/Node | Measure final packages; update existing descriptors, wrappers, notices and actual consumers together. Prove formulas and layout, policy rejection, and honest missing-codec errors. |
| P1 | Sustainable Flutter target-asset acquisition and publication readback | Normal, cached/offline and explicit local override builds consume exact target bytes; a real packaged app executes; the published pub archive is reconciled. |
| P1 | Node Linux ARM64 and complete browser full/render formula qualification | Real native/WASM/bundler/Worker consumers and measured compressed bytes, initialization and memory. No budget relaxation merely to admit a candidate. |
| P2 | Marketplace promotion, Typst formula product, optional Go wrapper or C package-manager recipes | Select based on actual user demand and host constraints, with one accountable existing/narrow owner per added channel. |

Each implementation can be a bounded main-line change. Do not multiply every binding by
every native feature combination, change all ABIs together, or build a general-purpose
release framework. Do not change v0.9.x or overwrite historical artifacts as part of this
study. The next implementation plan should pick the first coherent delivery slice and
its consumer proof, rather than require all channels to migrate before any user benefits.

## Evidence and limits

Internal authorities include [package surfaces](../release/PACKAGE_SURFACES.md),
[release owners](../release/RELEASING.md), [prebuilt choices](../release/PREBUILT_PRODUCTS.md),
[artifact recipes](../../capabilities/artifact-profiles-v2.json),
[native/WASI SDK integration](../../distribution/sdk/README.md), and the respective platform
guides. Main is inspected at the source identity above; mutable upstream sources describe
what was visible on the research date.

This study used repository inspection, official registry metadata, and first-party project
documentation/source. It did not rebuild packages, install every public registry artifact,
create publishing identities, dispatch workflows, or publish new versions. Historical
qualification from PR #265 is accepted groundwork rather than a test repeated here.
Installation examples for remote SwiftPM, Central, target-downloading Flutter hooks and
new math-enabled host packages are proposals until their owners implement and qualify them.

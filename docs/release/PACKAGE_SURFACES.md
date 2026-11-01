# Package Surfaces

Choose a package or artifact for the environment where you use Merman. Match generated wrappers
and native or WASM artifacts to the same Merman version.

For ordinary prebuilt use, start with [host and output choices](PREBUILT_PRODUCTS.md).
The main-line pilot recommends complete Apple SVG and the existing native C/WASI full SDKs;
expert source features and reduced SDK recipes remain available. The new Apple package is
unreleased and does not change published v0.9.x assets.

## Release versions

Merman `0.9.3` is the latest published patch on `0.9.0` and retains Mermaid `12.1.0`, parser `2.0.1`, and reference CLI
`12.0.0` compatibility. See the [0.9.3 release notes](README.md#merman-093) for its selected fixes
and the [0.8.0 to 0.9.0 upgrade guide](V080_TO_V090_UPGRADE_GUIDE.md) for the preceding API,
command, and output changes.

The development baseline selects Mermaid `12.2.0`, retaining parser `2.0.1` and reference CLI
`12.0.0`. Its source and artifact admission are recorded in [ADR-0094](../adr/0094-mermaid-12-2-upstream-transition.md).
This transition does not change package versions, feature ownership, or transport schemas.

The development baseline uses Options JSON schema `3` and Web transport API `8`, including
prepared sources and mounted diagrams. Those contracts and Mermaid `12.2.0` behavior are
unreleased. Main's package metadata records the `0.9.3` version baseline, but only immutable
`v0.9.3` identifies the published source and its artifacts. The Rust crates, CLI/LSP, and native
C/WASI SDKs and Apple XCFramework have verified `0.9.3` delivery; Web, Node, Python, Flutter, and Android retain
their separately published `0.9.2` packages.

Independently versioned packages include `roughr-merman 0.12.4` and Tree-sitter `0.3.1`. Typst wrapper `0.5.0` bundles Merman `0.9.0`; wrapper `0.4.0` bundles `0.8.0`. See the [dependency policy](../DEPENDENCY_POLICY.md) for compatible version requirements and intentional upgrade exceptions.

## Choose A Surface

| Need | Entry point | Delivery |
| --- | --- | --- |
| Parsing and typed models without rendering | `merman-core` | crates.io |
| Diagnostics, linting, Markdown/MDX scanning, and editor facts | `merman-analysis` | crates.io |
| Complete Rust rendering facade | `merman` | crates.io |
| A command-line renderer, linter, and exporter | `merman-cli` | GitHub Release archive or crates.io |
| A ready-to-run language server | `merman-lsp` | GitHub Release archive or crates.io |
| Raw Mermaid CST, incremental parsing, and syntax queries | `tree-sitter-mermaid` on crates.io or `@mermanjs/tree-sitter-mermaid` on npm | Independent grammar release |
| Checked Rustdoc fragments with no consumer renderer dependency | `merman-cli rustdoc` | Default source CLI and standard release archive |
| One-step Rustdoc Mermaid attributes | `merman-rustdoc` | crates.io |
| Browser SVG, analysis, ASCII, or editor SDK | one `@mermanjs/web*` package | npm package group |
| Native Node.js / static-site SVG rendering | `@mermanjs/node` | npm package group (alpha) |
| Python host integration | `merman` | PyPI and release wheels |
| Flutter/Dart host integration | `merman` | pub.dev |
| Android host integration | `merman-android-<tag>.aar` | GitHub Release AAR |
| Apple host integration | Complete `Merman-swiftpm-v<version>.zip` on main; legacy XCFramework route for v0.9.3 | Self-contained local SwiftPM package; separate binary-target ZIP |
| Language-neutral native embedding with explicit fonts and binary exports | `sdk-native-full` recommended | C headers, static/shared libraries, CMake and pkg-config; reduced `standard`/`math` available |
| Non-JavaScript sandbox embedding | `sdk-wasi-full` recommended | `wasm32-wasip1` reactor with independent scalar ABI 1; reduced `standard`/`math` available |
| Binding source integration | `merman-bindings-core`, `merman-ffi`, `merman-uniffi`, `merman-wasm` | crates.io source crates; native artifacts are separate owner surfaces |
| Typst plugin package | `distribution/typst/merman` | manual Typst registry submission |
| VS Code integration | `merman-vscode` | GitHub Actions VSIX artifact |

Foundational Rust implementation crates are not product entry points. Homebrew/core owns formula
publication; this repository only validates the external formula after a stable release.

Android, Python, and Flutter retain their default prebuilt native capability SKU: SVG, semantic
and layout operations, both supported layout engines, ASCII, analysis, validation, and document
analysis. Main's Apple product additionally selects math explicitly; published v0.9.x Apple
artifacts retain the former shared math-free selection. PNG, JPEG, PDF, and native runtime adapters
remain available to custom source builds but are not bundled in these default packages. These platform artifacts intentionally include
ELK and therefore ship under an artifact-specific notice closure; they are not equivalent to the
default `merman` Cargo feature. The C ABI crate has no default features, so custom embedders can
select semantic-only, SVG-only, export-capable, or complete builds. LSP remains a separate
executable product and is not linked into any native binding artifact.

## Headless SDK Profiles

The SDK archive recipes add a separate delivery surface. Their recommended full entry point
does not change the meanings of standard or math, or the other language packages. Archives are release-qualified
by their owner workflow, not implied to exist for an already published version.

| Profile | Math | ELK | Native outputs | WASI outputs |
| --- | --- | --- | --- | --- |
| `standard` | No | No | SVG, PNG, JPEG, PDF, ASCII | SVG, PNG |
| `math` | Yes | No | SVG, PNG, JPEG, PDF, ASCII | SVG, PNG |
| `full` | Yes | Yes | SVG, PNG, JPEG, PDF, ASCII | SVG, PNG |

Native archives target Apple ARM64/x86-64, GNU Linux ARM64/x86-64 and Windows x86-64
MSVC; each includes both shared and static libraries. WASI uses `wasm32-wasip1`, without
JavaScript, browser APIs, preopened directories or ambient font discovery. Its ABI is not
the native function-pointer table. Standard and math preserve Mermaid's family fallback
when ELK is not compiled; the catalog still reports ELK as absent. Math absence is a typed
capability error. Math/full use the supported interactive resource policy and host limits;
explicit constrained math requests remain rejected.

Both transports can own caller-supplied font bytes and share them between measurement and
painting. Bundled Cousine is a limited fallback, not full CJK/emoji coverage. The SVG keeps
text rather than embedding those fonts; see the [output guide](../rendering/RASTER_OUTPUT.md).
Every archive records its actual catalog, target, ABI/options schemas, font inventory,
checksums and corresponding source commit. Standard is not "MIT-only": dependency MPL
and font OFL notices still apply. Math adds its own renderer/font notices; full additionally
includes EPL/ELK material and source-availability information.

See [SDK integration and packaging](../../distribution/sdk/README.md) and the
[real WASI host examples](../../platforms/wasi/README.md). On `release/0.9.x`, this SDK
surface is a maintainer-authorized additive maintenance exception: Mermaid 12.1, Options
schema 2, old ABI prefixes and native codec discovery semantics remain unchanged. Main
retains its separate development baseline and Options schema 3. Match artifacts to the
same source line and version rather than mixing wrappers from the two lines.

## Rustdoc Documentation

Rustdoc support is intentionally two product surfaces, not one package with hidden backend
selection. `merman-cli rustdoc build/check` owns checked, committed Markdown fragments and a
portable receipt; consuming crates use native `include_str!`, and docs.rs only reads files already
inside the uploaded crate. `merman-rustdoc` owns item-level attribute expansion and compiles its
native renderer while `cargo doc` runs. The CLI never depends on or invokes the macro package, and
the macro never discovers or invokes the CLI. Both use the small `merman-doc` implementation
library for Markdown recognition and HTML embedding, and the renderer owns SVG safety and ID
isolation. The shared library does not depend on either product or select rendering features; it
is published in the workspace dependency order before both consumers.

The macro and current-source facade default to `all-diagrams`, `svg`,
`layout-cytoscape`, and `layout-elk`; mathematical labels require explicit `math`.
The current development branch retains `complete-svg` (SVG and Cytoscape) and `complete-svg-elk`
(also ELK) on both packages, without math or implicit diagram selection. Add `math` explicitly
when upgrading a previously math-enabled aggregate selection; installed older products retain
their original feature definitions.
Both source defaults include the EPL-2.0 ELK implementation and its notice/source obligations.
To omit ELK, disable default features and select the required diagram families plus `svg`
and required layout leaves. Keep the macro dependency optional when ordinary builds should
avoid its renderer closure; use generated fragments when documentation builds must avoid it too.
[ADR-0088](../adr/0088-lean-rustdoc-default-features.md) records the earlier math opt-in decision;
the alpha.7 Mermaid 12 alignment adds ELK to the product default.

| Distribution property | `merman-cli rustdoc` | `merman-rustdoc` |
| --- | --- | --- |
| Consumer Cargo graph | No attributable Merman renderer, layout, math, or proc-macro package | Selected proc-macro and renderer closure |
| Release recipe | Included in CLI defaults and the `cli-release` artifact recipe | Independent `rustdoc-static-svg` artifact profile |
| Published inputs | Config, source, generated fragments, and `receipt.json` | Annotated Rust source and optional included `.mmd` files |
| Hosted documentation | Packaged fragments work offline without executing the CLI | docs.rs must enable the optional documentation feature |
| Freshness/rollback | CI runs the default CLI `rustdoc check`; Git owns successful-state rollback | `cargo doc` fails during expansion; Git owns source rollback |

Generated fragments must be included at most once on a rendered page to avoid duplicate static DOM
IDs. Package preflight must prove every referenced fragment and the receipt are present. The
[`merman-cli` Rustdoc guide](../../crates/merman-cli/README.md#rustdoc-fragments) and
[`merman-rustdoc` guide](../../crates/merman-rustdoc/README.md) contain the runnable workflows.

## Release Delivery

The repository-owned delivery routes are:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli` and `merman-lsp`.
3. Native C and WASI headless SDK archives attached manually to an existing GitHub Release.
4. GitHub Release XCFramework packaging for Apple.
5. GitHub Release wheels and PyPI publishing for Python.
6. pub.dev for Flutter.
7. GitHub Release AAR for Android.
8. lockstep npm publishing for the admitted `@mermanjs/web` browser package group through
   `release-web.yml` after Trusted Publishing setup.
9. lockstep npm publishing for `@mermanjs/node`, its five native platform packages, and the
   explicit `@mermanjs/node-wasm` package through `release-node.yml`. Node is an experimental alpha
   surface; it requires first-publish bootstrap before npm Trusted Publishing can take over.
10. Platform VSIX artifacts for the independently versioned VS Code extension through
   `vscode-extension.yml`; Marketplace publishing needs an explicit release decision and credentials
   before it is enabled.
11. Independent Cargo, scoped npm, and GitHub publication for `tree-sitter-mermaid` through
    `release-tree-sitter-mermaid.yml` after both registry identities are bootstrapped.
12. Typst Universe submission for `@preview/merman:0.5.0`, bundling Merman `0.9.0`; wrapper `0.4.0` bundles Merman `0.8.0`. The wrapper has an independent version axis and is not published by the crates.io workflow or a workspace tag.

The source-crate route and native-artifact routes must be audited separately. Publishing
`merman-ffi` or `merman-uniffi` to crates.io does not publish an Android AAR, Apple XCFramework,
Python wheel, or Flutter package. Use `scripts/release_surface_contract.py` to enumerate the owner
workflows and artifact profiles before treating a release as complete.

## CI Gates

Merman CI keeps publication separate from validation:

- `cargo run -p xtask -- verify-mermaid-reference` checks that the selected Mermaid and companion
  behavior graph, package locks, generated runtime labels, and provenance agree.
- `cargo run -p xtask -- verify-editor-language-contract` checks parser-owned rename-policy and
  analysis-setting projections. Tree-sitter highlighting uses its ordinary grammar/query tests and
  adapter tests; it has no generated packed-token evidence contract.
- `cargo run -p xtask -- verify-web-diagram-catalog` and
  `verify-playground-example-catalog` keep the published full/editor family set and source-backed
  examples aligned.
- `platform-script-syntax` checks Python, Apple, and Flutter shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- The platform binding gate runs Flutter dependency resolution, generated-binding drift checks, static analysis, formatting, ABI contract tests, and a native Dart smoke against the exact desktop artifact recipe. Release preflight also rejects a package whose Dart-reported compressed size exceeds 99 MB.
- `apple-uniffi-smoke` builds `Merman.xcframework` and validates the generated UniFFI Swift package.
- `web-npm-dry-run` builds each admitted TypeScript/WASM package, verifies its package projection,
  then packs and verifies the complete lockstep npm group without publishing it.
- `release-node.yml` builds, packs, installs, and renders the public Node loader through its real
  macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 native packages, plus the explicit
  Node-targeted WASM package. Its publisher receives a verified seven-package group only and
  publishes platform packages, WASM, and then the root loader under the requested final dist-tag.
- `vscode-extension.yml` and the VS Code preflight job build platform runtime binaries, package a
  VSIX, and verify package contents, target platform, stable manifest version, and pre-release
  marker.
- `homebrew.yml` checks the published stable Homebrew formula on a schedule or on demand, runs
  `brew livecheck`, installs the exact formula version, and exercises native rendering. Formulae
  implementing CLI contract v2 are also checked for capability JSON and generated completions;
  every installed formula runs Homebrew's linkage audit and formula test.

Release preflight is manual and publish-free. Crates and cargo-dist remain tag-driven after
preflight passes. Platform publishing is manual so a fixed workflow on `main` can build and upload
assets for an existing release tag without moving that tag. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.

## Browser WASM Package Group

`platforms/web` is a private build workspace, not an npm product. The public browser SDK is a
lockstep package group described by `platforms/web/web-surface-descriptor.json`. Each published
package exports only `.` and contains exactly one owned WASM artifact at
`artifacts/wasm/merman_wasm_bg.wasm`; raw `pkg/**` files and package subpaths are not public API.

| Package | Artifact profile | Intended use |
| --- | --- | --- |
| `@mermanjs/web` | `web-full` | Browser SDK: SVG, analysis, ASCII, editor intelligence, Cytoscape and ELK layouts. |
| `@mermanjs/web-analysis` | `web-analysis` | Analysis, diagnostics, facts, and detection without rendering. |
| `@mermanjs/web-editor` | `web-editor` | Analysis plus parser-backed editor intelligence, intended for a dedicated Worker. |
| `@mermanjs/web-ascii` | `web-ascii` | ASCII/Unicode diagram output. |
| `@mermanjs/web-render` | `web-render` | SVG-only renderer with Cytoscape and ELK layouts. |

The package group deliberately has no Node or SSR fallback. Browser consumers import one package,
then use its generated wrapper and runtime capability report. Unsupported operations are rejected by
the artifact's typed missing-capability behavior; consumers should not infer support from package
names or Cargo features.

The generic browser packages omit math. Required math labels return the typed missing-capability
error with id `math`; custom source builds can select the positive `math` feature explicitly.
See the [Rust embedding and capability guide](../rendering/RUST_EMBEDDING.md).

Release builds pack every public member into one verified artifact group. The manifest records the
release version, source commit, target dist-tag, package/profile identity, tarball hash and npm
integrity, and legal-material digest. The publisher preflights every existing version and target tag,
then publishes missing versions directly under the final tag in dependency order. npm Trusted
Publisher cannot perform a later `dist-tag` mutation, so a conflicting existing tag stops the run
before any publish; a retry safely skips members whose integrity and tag already match. npm cannot
provide cross-package transactions, so the root package is deliberately published last.

## Compatibility And Migration Notes

Current source semantics are intentionally explicit:

- Cargo features separately select positive diagram families and capabilities; the source of truth
  for an exact shipped artifact is the artifact profile catalog. The Rust facade defaults to
  `all-diagrams + svg + layout-cytoscape + layout-elk` (math is opt-in). Consumers disabling
  defaults must select `all-diagrams` or the required `diagram-*` selectors alongside output leaves
  and layout leaves. Products and release profiles select explicit families
  and capabilities when they need an artifact-specific closure.
- Native bindings use ABI 3. Hosts must query the generated capability/runtime catalog before
  requesting optional output or a host text-measurement operation, and must reject an ABI mismatch at
  initialization rather than relying on struct layout compatibility.
- Browser package identity is part of the public API. Migrate old
  `@mermanjs/web/<subpath>` imports to the matching standalone package in the table above. There is
  no compatibility subpath or raw WASM fallback.
- `merman-wasm` is the wasm-bindgen implementation crate published through crates.io. It is not a
  browser package, a Node module, or evidence that a Typst artifact supports the same capability set.
- A new public browser package, changed default package, or candidate admission requires a separate
  release decision with a descriptor entry, artifact profile, package-size evidence, independent
  installation smoke, and a migration note.

## Native Prebuilt SKU Policy

Python, Android, and Flutter retain the same default native prebuilt SKU through
surface-owned transports. Its direct features are `analysis`, `ascii`, `layout-cytoscape`,
`layout-elk`, and `svg`; its outputs are ASCII and SVG. It omits math, binary export, and native
runtime adapters to keep common downloads materially smaller. The generated wrappers still expose
the complete operation vocabulary, while runtime discovery and typed missing-capability errors
describe the loaded artifact precisely.

Main's Apple recipe adds math explicitly for complete SVG document semantics. Its distribution
copies omit delivery-only bitcode without changing Rust LTO settings or original build outputs.
See [ADR-0100](../adr/0100-prebuilt-product-entry-points.md). This main-only amendment leaves
source defaults and published v0.9.x products unchanged.

The C ABI is published as the source-only `merman-ffi` crate. Its `c-abi-native` artifact profile
continues to build the complete host reference library for ABI and output-path verification, not a
downloadable default binary SDK. The binding crates intentionally have no `complete-svg` aggregate:
source users select direct leaves such as `svg`, `layout-cytoscape`, `layout-elk`, `math`, and
`native-runtime`, then use the artifact recipe and notices as the legal closure authority.

The corresponding cross-language recipes are intentionally not identical when their interfaces
differ:

| Surface | Compiled capabilities | Product rationale |
| --- | --- | --- |
| Android, Python, Flutter | analysis, ASCII, SVG, Cytoscape, ELK | Existing native prebuilt defaults. |
| Apple on main | analysis, ASCII, SVG, math, Cytoscape, ELK | Complete SVG local SwiftPM package; published v0.9.x remains math-free. |
| Typst | analysis, SVG, Cytoscape, ELK | Matches the six-function Typst ABI, whose SVG-capable transport also exposes three explicitly Alpha theme-authoring operations; no callable ASCII or binary-export operation, and no admitted math backend. |
| Node alpha package group | SVG, Cytoscape, ELK | Matches the deterministic static-SVG interface; specialist capabilities remain out of the prebuilt download. |
| Browser packages | package-specific | `web-full` and `web-render` omit math; dedicated packages own analysis, editor, and ASCII workflows. Custom source builds can opt into math. |
| C ABI source reference | complete | Exercises every ABI/output path for custom embedders without defining a default binary download. |

Do not add another prebuilt matrix solely because another source closure compiles. A proposed SKU
must identify a distinct consumer workflow and compare final artifacts from the same revision,
target, toolchain, and machine. It must also own package naming, installation smokes, legal closure,
update policy, and release CI. See ADR-0079 for the accepted default boundary and its matched
library-size evidence.

## Release Gates By Surface

`docs/release/RELEASING.md` owns the operator sequence and cross-surface commands. Each package, artifact profile, descriptor, and release workflow owns its direct build and publication evidence. This document records what each gate must prove:

| Surface | Required evidence |
| --- | --- |
| Browser npm package group | Pinned Mermaid and editor descriptors, exact artifact profiles, TypeScript/WASM contracts, package projection, install smoke, lockstep group assembly, and provenance all agree. |
| VS Code extension | Descriptor-owned LSP and CLI artifacts pass extension tests, binary preparation, target-specific VSIX packaging, and package-content verification. |
| Browser artifact evidence | The selected Web artifact profiles have current raw, stripped, gzip, and Brotli measurements; do not substitute a legacy feature-profile name. |
| Browser/Typst size evidence | The owner-specific Web and Typst size commands share one budget catalog and together cover every admitted artifact exactly once. |
| Typst transport | The sole `publish` package profile consumes the canonical `typst-wasm` artifact recipe and proves plugin ABI 3, dependency closure, size, provenance, package contents, and examples. Its admitted `json5`, `html5ever`, and `url` dependencies remain measured pure-Rust parts of invariant Mermaid semantics. |
| Node npm alpha package group | The selected N-API recipe, explicit Node-targeted WASM recipe, generated wire contract, runtime catalog, package contracts, exact-version optional dependencies, glibc-baseline build receipts, packed tarballs, five native install/render smokes, and one WASM install/render smoke agree. |

## WASM Size Matrix

Use the xtask size matrix before changing an artifact profile:

```bash
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
cargo run -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface typst \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The Web command measures the wasm-bindgen artifacts inside the assembled npm package directories;
the preceding smoke proves their provenance and package closure. It must not rebuild an alternate
Cargo-only size artifact. The Typst
command builds its descriptor-owned `wasm-size` artifact because that transport has a separate
package producer. Both report raw, stripped, gzip, and Brotli bytes together with the exact Cargo
profile, target, feature set, runtime IDs, capabilities, and output IDs. The schema-2 budget file
must cover every exact Web and Typst artifact profile once, with no legacy feature-profile entries
or stale profiles. Compare only artifacts with the same profile and target; browser and Typst
transports deliberately have different closures.

The 2026-08-03 complete-SVG admission refresh measured the final `web-render` npm WASM against the
capability-superset `web-full` artifact produced by the same wasm-pack toolchain:

| Artifact profile | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `web-full` | 12,584,849 | 12,584,626 | 4,787,991 | 3,406,733 |
| `web-render` | 11,844,334 | 11,844,111 | 4,500,248 | 3,195,427 |

Removing analysis, ASCII, and editor saves 5.88% by stripped bytes and 6.20% by Brotli. The package
is admitted because it establishes the complete SVG-only capability contract, not because it meets
the 15% threshold used for workflow-specific slim packages. Do not weaken `web-render` to basic SVG
under the same package identity: it would no longer be capability-equivalent. A future basic-SVG
package needs a separate workflow and at least a 15% measured reduction against its declared
comparison artifact before release admission is considered.

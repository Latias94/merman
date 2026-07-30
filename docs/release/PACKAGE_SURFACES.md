# Package Surfaces

This guide describes the supported package and artifact choices. It is not a live registry-status
database; verify a specific version at its owning registry or GitHub Release before recommending an
installation command.

## Choose A Surface

| Need | Entry point | Delivery |
| --- | --- | --- |
| Parsing and typed models without rendering | `merman-core` | crates.io |
| Diagnostics, linting, Markdown/MDX scanning, and editor facts | `merman-analysis` | crates.io |
| Complete Rust rendering facade | `merman` | crates.io |
| A command-line renderer, linter, and exporter | `merman-cli` | GitHub Release archive or crates.io |
| A ready-to-run language server | `merman-lsp` | GitHub Release archive or crates.io |
| Rustdoc Mermaid fences | `merman-rustdoc` | crates.io |
| Browser SVG, analysis, ASCII, or editor SDK | one `@mermanjs/web*` package | npm package group |
| Python host integration | `merman` | PyPI and release wheels |
| Flutter/Dart host integration | `merman` | pub.dev |
| Android host integration | `io.merman:merman-android` | GitHub Release AAR |
| Apple host integration | `Merman.xcframework` | GitHub Release asset or local SwiftPM package |
| Typst plugin package | `packages/typst/merman` | manual Typst registry submission |
| VS Code integration | `merman-vscode` | GitHub Actions VSIX artifact |

Foundational Rust implementation crates are not product entry points. Homebrew/core owns formula
publication; this repository only validates the external formula after a stable release.

## Release Delivery

The repository-owned delivery routes are:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli` and `merman-lsp`.
3. GitHub Release XCFramework packaging for Apple.
4. GitHub Release wheels and PyPI publishing for Python.
5. pub.dev for Flutter.
6. GitHub Release AAR for Android.
7. lockstep npm publishing for the admitted `@mermanjs/web` browser package group through
   `release-web.yml` after Trusted Publishing setup.
8. Platform VSIX artifacts for the independently versioned VS Code extension through
   `vscode-extension.yml`; Marketplace publishing needs an explicit release decision and credentials
   before it is enabled.

## CI Gates

Merman CI keeps publication separate from validation:

- `cargo run -p xtask -- verify-mermaid-reference` checks that the selected Mermaid and companion
  behavior graph, package locks, generated runtime labels, and provenance agree.
- `cargo run -p xtask -- verify-editor-token-descriptor` checks the single editor-language token
  descriptor, its Rust/Web/VS Code projections, the generated VS Code token/theme contributions,
  and the exact 35-family plus recovery packed-token evidence before LSP or browser packages build.
- `cargo run -p xtask -- verify-web-diagram-catalog` and
  `verify-playground-example-catalog` keep the published full/editor family set and source-backed
  examples aligned.
- `platform-script-syntax` checks Python, Apple, and Flutter shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- The platform binding gate runs Flutter dependency resolution, generated-binding drift checks, static analysis, formatting, ABI contract tests, and a native Dart smoke against the exact desktop artifact recipe.
- `apple-uniffi-smoke` builds `Merman.xcframework` and validates the generated UniFFI Swift package.
- `web-npm-dry-run` builds each admitted TypeScript/WASM package, verifies its package projection,
  then packs and verifies the complete lockstep npm group without publishing it.
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
| `@mermanjs/web` | `web-full` | Complete browser SDK: SVG, analysis, ASCII, editor intelligence, Cytoscape and ELK layouts, and math. |
| `@mermanjs/web-analysis` | `web-analysis` | Analysis, diagnostics, facts, and detection without rendering. |
| `@mermanjs/web-editor` | `web-editor` | Analysis plus parser-backed editor intelligence, intended for a dedicated Worker. |
| `@mermanjs/web-ascii` | `web-ascii` | ASCII/Unicode diagram output. |
| `@mermanjs/web-render` | `web-render` | Public complete SVG-only renderer with Cytoscape, ELK, and math. |

The package group deliberately has no Node or SSR fallback. Browser consumers import one package,
then use its generated wrapper and runtime capability report. Unsupported operations are rejected by
the artifact's typed missing-capability behavior; consumers should not infer support from package
names or Cargo features.

Release builds pack every public member into one verified artifact group. The manifest records the
release version, source commit, target dist-tag, package/profile identity, tarball hash and npm
integrity, and legal-material digest. Publication first makes every exact version available under a
staging tag, then promotes the requested public dist-tag only after the complete group verifies. A
promotion failure restores previously changed tags and leaves a reconciliation report for a safe
rerun. npm cannot provide cross-package transactions; this is the strongest recoverable boundary.

## Compatibility And Migration Notes

Current release semantics are intentionally explicit:

- Cargo features describe positive capabilities; the source of truth for an exact shipped artifact
  is the artifact profile catalog, not historical `full`, `tiny`, or per-diagram feature aliases.
  The Rust facade keeps only the result-named `complete-svg` convenience aggregate; products and
  release profiles select direct leaf features.
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

Python, Apple, Android, and Flutter releases currently ship one full prebuilt native SDK SKU per
surface. The C ABI is published as the source-only `merman-ffi` crate; its native artifact profile
defines reproducible host reference libraries, not a downloadable binary SDK. These are
intentional product boundaries, not inferences from the Rust facade default. Source users can
select `complete-svg` or direct feature leaves; release recipes select the full leaf set explicitly
through their artifact profiles.

Do not add a `full`/`svg` prebuilt matrix solely because both source closures compile. A proposed
SVG-only native SKU must identify concrete consumer demand and set its material-improvement
threshold before collecting evidence. The proposal must compare the existing full artifact and the
candidate from the same revision, target, toolchain, and machine; measure final stripped library or
framework bytes and packaged artifact bytes; run the existing installation and output smokes; and
record runtime memory only where the selected platform has a stable, representative measurement
method. The proposal must also own package naming, legal closure, update policy, and CI for the
extra SKU.

As of 2026-07-26, the repository has no comparable all-target record of final package bytes,
smoke-process peak RSS, and cold-start behavior for a complete-SVG candidate versus each full
native SDK. Therefore no second native SKU is admitted by this refactor; source-level Cargo
closure differences alone are not release evidence. Keep this comparison in the proposal that
introduces the candidate; do not add a standing PR or release gate until maintainers have accepted
a second SKU and a stable budget.

## Release Gates By Surface

`docs/release/RELEASING.md` owns the operator sequence and cross-surface commands. Each package, artifact profile, descriptor, and release workflow owns its direct build and publication evidence. This document records what each gate must prove:

| Surface | Required evidence |
| --- | --- |
| Browser npm package group | Pinned Mermaid and editor descriptors, exact artifact profiles, TypeScript/WASM contracts, package projection, install smoke, lockstep group assembly, and provenance all agree. |
| VS Code extension | Descriptor-owned LSP and CLI artifacts pass extension tests, binary preparation, target-specific VSIX packaging, and package-content verification. |
| Browser artifact evidence | The selected Web artifact profiles have current raw, stripped, gzip, and Brotli measurements; do not substitute a legacy feature-profile name. |
| Browser/Typst size evidence | The combined size matrix covers every admitted Web and Typst artifact exactly once. |
| Typst transport | The sole `publish` package profile consumes the canonical `typst-wasm` artifact recipe and proves plugin ABI 2, dependency closure, size, provenance, package contents, and examples. Its admitted `json5`, `lol_html`, and `url` dependencies remain measured pure-Rust parts of invariant Mermaid semantics. |

## WASM Size Matrix

Use the xtask size matrix before changing an artifact profile:

```bash
cargo run -p xtask -- wasm-size-matrix --surface web --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface all --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The command builds declared `wasm-size` artifacts and reports raw, stripped, gzip, and brotli
bytes together with the exact Cargo profile, target, feature set, runtime IDs, capabilities, and
output IDs. The schema-2 budget file must cover every exact Web and Typst artifact profile once,
with no legacy feature-profile entries or stale profiles. Compare only artifacts with the same profile and
target; browser and Typst transports deliberately have different closures.

The 2026-07-26 complete-SVG admission run measured `web-render` against the capability-superset
`web-full` on the same `wasm32-unknown-unknown` `wasm-size` profile:

| Artifact profile | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `web-full` | 18,284,917 | 14,392,820 | 4,646,814 | 3,215,042 |
| `web-render` | 17,023,368 | 13,543,476 | 4,379,667 | 3,043,714 |

Removing analysis, ASCII, and editor saves 5.90% by stripped bytes and 5.33% by Brotli. The package
is admitted because it establishes the complete SVG-only capability contract, not because it meets
the 15% threshold used for workflow-specific slim packages. Do not weaken `web-render` to basic SVG
under the same package identity: it would no longer be capability-equivalent. A future basic-SVG
package needs a separate workflow and at least a 15% measured reduction against its declared
comparison artifact before release admission is considered.

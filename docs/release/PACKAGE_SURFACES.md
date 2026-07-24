# Package Surfaces

Status: maintained release surface contract.
Last updated: 2026-07-23

This document records merman package surfaces, current readiness, and the CI gates that protect
each publication or artifact build.

## Current Surfaces

<!-- BEGIN GENERATED RELEASE SURFACES -->
| Contract ID | Surface | Entry point | Support | Channels |
| --- | --- | --- | --- | --- |
| `rust-core` | Rust core crate | `merman-core` | `published` | `crates.io` (`published`) |
| `rust-analysis` | Rust analysis crate | `merman-analysis` | `published` | `crates.io` (`published`) |
| `rust-render` | Rust render facade | `merman` | `published` | `crates.io` (`published`) |
| `rust-editor-lsp` | Rust editor and LSP crates | `merman-lsp` | `published` | `github-release` (`published`), `crates.io` (`published`) |
| `rustdoc` | Rustdoc integration | `merman-rustdoc` | `published` | `crates.io` (`published`) |
| `cli` | Command line interface | `merman-cli` | `published` | `github-release` (`published`), `crates.io` (`published`) |
| `homebrew` | Homebrew CLI formula | `merman-cli` | `stable-only` | `homebrew-core` (`published`) |
| `web-package-group` | Browser WebAssembly package group | `@mermanjs/web` | `published` | `npm` (`published`) |
| `vscode` | VS Code extension | `merman-vscode` | `artifact-only` | `github-actions-vsix` (`artifact-only`), `vs-marketplace` (`credential-blocked`) |
| `c-abi` | Native C ABI | `merman-ffi` | `published` | `crates.io` (`published`) |
| `python` | Python UniFFI package | `merman` | `published` | `pypi` (`published`), `github-release-wheels` (`published`), `crates.io` (`published`) |
| `flutter` | Flutter and Dart FFI package | `merman` | `published` | `pub.dev` (`published`) |
| `android` | Android JNI package | `io.merman:merman-android` | `artifact-only` | `github-release-aar` (`artifact-only`), `maven-central` (`credential-blocked`) |
| `apple` | Apple Swift package and XCFramework | `Merman` | `artifact-only` | `github-release-xcframework` (`artifact-only`), `swiftpm-remote-binary` (`registry-blocked`) |
| `typst` | Typst package and WASM plugin | `packages/typst/merman` | `manual-registry` | `typst-registry` (`manual-registry`), `crates.io` (`published`) |
<!-- END GENERATED RELEASE SURFACES -->

The generated table is the public view of `SURFACES.json`; refresh it with
`python scripts/verify-release-surfaces.py --write-docs`. Foundational Rust crates are intentionally
hidden from this user-choice table while remaining part of the maintainer and crates.io contract.
Homebrew/core owns formula publication. Merman verifies the external stable formula rather than
claiming to publish it from this repository. Registry-blocked and credential-blocked channels are
not silently presented as available install paths.

## Release Surface Set

The repository-owned release surface set is:

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

## Release Status States

`docs/release/SURFACES.json` is the machine-readable source of truth for public package surfaces and
release channels. The status terms are intentionally user-facing:

| State | Meaning |
| --- | --- |
| `published` | The registry or install channel is expected to publish for the selected release kind. |
| `artifact-only` | CI produces or uploads an artifact, but no registry package is published from this repo yet. |
| `credential-blocked` | The registry path is designed but blocked on credentials, signing, or marketplace setup. |
| `registry-blocked` | The registry package contract needs more release-manifest design before publication. |
| `manual-registry` | Publication happens through a manual registry PR or external review flow. |
| `not-built` | The surface is documented but not produced by current automation. |
| `not-applicable` | The channel does not apply to the selected release kind, such as Homebrew for prereleases. |

For a user-facing package-choice table:

```bash
python scripts/release-status.py --view public
```

For maintainer readiness against a candidate version:

```bash
VERSION="<version>"
python scripts/release-status.py --version "$VERSION" --view maintainer
python scripts/release-status.py --version "$VERSION" --probe --format json
```

`--probe` is best-effort and should be used after publication when network and registry tools are
available. It reports observed status separately from the declared release state.

## CI Gates

Merman CI keeps publication separate from validation:

- `python scripts/verify-release-surfaces.py` checks `SURFACES.json`, package manifests, the closed
  Web package descriptor, public/candidate ownership, and release workflow operations. It does not
  treat prose wording as a build gate.
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
- `flutter-package-check` runs `flutter pub get`, `flutter analyze`, and Dart formatting.
- `apple-uniffi-smoke` builds `Merman.xcframework` and validates the generated UniFFI Swift package.
- `web-npm-dry-run` builds each admitted TypeScript/WASM package, verifies its package projection,
  then packs and verifies the complete lockstep npm group without publishing it.
- `vscode-extension.yml` and the VS Code preflight job build platform runtime binaries, package a
  VSIX, and verify package contents, target platform, stable manifest version, and pre-release
  marker.
- `homebrew.yml` checks the published Homebrew formula, runs `brew livecheck`, installs
  `merman-cli`, and renders a smoke diagram from the installed binary.

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
| `@mermanjs/web` | `web-full` | Complete browser SDK: SVG, analysis, ASCII, editor intelligence, Cytoscape and ELK layouts. |
| `@mermanjs/web-analysis` | `web-analysis` | Analysis, diagnostics, facts, and detection without rendering. |
| `@mermanjs/web-editor` | `web-editor` | Analysis plus parser-backed editor intelligence, intended for a dedicated Worker. |
| `@mermanjs/web-ascii` | `web-ascii` | ASCII/Unicode diagram output. |
| `@mermanjs/web-render` | `web-render` | Private candidate for SVG rendering. It is not published until independent admission proves its package-size and behavior contract. |

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

## Release Gates By Surface

| Surface | Required local gate before release changes |
| --- | --- |
| Browser npm package group | `cargo run -p xtask -- verify-mermaid-reference`; `cargo run -p xtask -- verify-editor-token-descriptor`; `cargo run -p xtask -- verify-artifact-profiles`; `npm run check:contracts --prefix platforms/web`; `npm run build --prefix platforms/web`; `npm run smoke --prefix platforms/web`; `npm run verify:packages --prefix platforms/web`; `python3 scripts/web_package_group.py validate-descriptor --descriptor platforms/web/web-surface-descriptor.json` |
| VS Code extension | `cargo build --release --locked --manifest-path crates/merman-lsp/Cargo.toml -p merman-lsp --bin merman-lsp --no-default-features --features stdio`; `cargo build --release --locked --manifest-path crates/merman-cli/Cargo.toml -p merman-cli --bin merman-cli --no-default-features --features analysis,ascii,jpeg,layout-cytoscape,layout-elk,math,network-icons,parallel-markdown,pdf,png,shell-completions,svg,system-clock,system-random,system-timezone,system-timing`; `npm run test --prefix tools/vscode-extension`; `npm run prepare:binaries --prefix tools/vscode-extension`; `npm run package --prefix tools/vscode-extension -- --target <target> --out <file>`; `npm run verify:vsix --prefix tools/vscode-extension -- --vsix <file> --platform <target> --target <target>` |
| Browser artifact evidence | `cargo run -p xtask -- wasm-size-matrix --surface web --budget-file docs/release/WASM_SIZE_BUDGETS.json`; inspect the selected artifact profile instead of a legacy feature-profile name. |
| Browser/Typst size evidence | `cargo run -p xtask -- wasm-size-matrix --surface all --budget-file docs/release/WASM_SIZE_BUDGETS.json` |
| Typst transport | `cargo run --locked -p xtask -- verify-typst-profile-constants`; `cargo run --locked -p xtask -- profile-budget check-deps --profile typst-wasm --artifact-profile typst-wasm`; `cargo run --locked -p xtask -- build-typst-package --profile publish`; `cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json`; `cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build`. The package builder consumes the sole `publish` profile and canonical `typst-wasm` artifact recipe, then validates the generated artifact, flat runtime catalog, plugin ABI `2`, size, package provenance, and Typst examples without exposing a private target path. PR CI compiles package examples and a preview import smoke with Typst 0.15.0, and push CI additionally runs the size matrix plus the tests-only package smoke. The dependency gate derives package, manifest, target, default-feature policy, and features from the exact artifact profile. Its admitted `json5`, `lol_html`, and `url` dependencies are pure-Rust parts of invariant Mermaid semantics and remain measured by the artifact size budget. |

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

# Publish Order

Status: maintained workspace publish order.
Last updated: 2026-09-02

## Version Decision

Published workspace prerelease baseline: `0.8.0-alpha.6`.

The workspace release is published from immutable tag `v0.8.0-alpha.6` at commit
`d529f858ea3d337a1bdc8fe12e44e1403ededf2e`. The crates.io workflow published all 20 workspace
crates, and the GitHub Release published the CLI/LSP archives and their verification assets. The
browser, Node.js, Flutter, Python, Apple, Android, and Typst package groups remain independent
publication tracks; their alpha.6 availability must be established by their owning workflow or
registry rather than inferred from the workspace tag. The browser and Node package groups were
previously published as an authorized alpha-channel test at `0.8.0-alpha.5` from reviewed commit
`d4365ca4860b6b4d51c421e775daab92a815c667`, newer than the workspace `v0.8.0-alpha.5` tag. Their
verified package-group manifests and workflow artifacts identify that commit. Because that first
publication was a manual bootstrap, those npm registry artifacts do not expose npm provenance
attestations; documentation must not imply either an attestation or cross-channel byte identity.

Rationale:

- crates.io versions are immutable and `0.8.0-alpha.1` has already started the 0.8 release line.
- The workspace has added 0.8-line Typst/package-size feature work and Mermaid parity fixes that
  should be tested behind a prerelease before the next stable cut.
- Workspace-coupled platform packages should stay aligned so downstream Web, FFI, and documentation
  integrations test one coherent version graph. The unpublished VS Code extension follows its own
  `0.1.x` version track and records the bundled workspace runtime separately.

Workspace Cargo manifests were published as `0.8.0-alpha.6`. Python package metadata uses the PEP
440 spelling `0.8.0a6`, but that version remains an independent publication decision; likewise,
separately published alpha.5 channels do not share a source snapshot with the workspace release by
implication. The independently versioned VS Code extension, Typst wrapper, and `roughr-merman`
remain on their own release axes. The `tree-sitter-mermaid` language distribution also has an
independent version axis.

For every prerelease, coupled workspace dependency requirements are exact (`=X.Y.Z-alpha.N`,
`=X.Y.Z-beta.N`, or `=X.Y.Z-rc.N`). This is a source-manifest rule, not a lockfile preference:
fresh consumers must never be allowed to select a newer sibling package for an older facade. Stable
workspace releases keep ordinary compatible requirements. The four binding source crates remain in
the crates.io graph; native platform bytes are delivered by their owner workflows and are not
implied by the presence of the source crates. Registry tarballs are immutable; if a published
prerelease contains a moving sibling requirement, the correction takes effect only in a later
release (or a new compatibility line), not by editing the already-published version in this tree.
Version `0.1.0` is published on crates.io and npm from tag `tree-sitter-mermaid-v0.1.0`, commit
`34ddaccbfb8b4a7a502e67122b2cd709b4989e19`. Its standalone GitHub Release is intentionally
deferred so it can be announced alongside the next Merman product release; the two releases retain
their own tags and version identities.

## Typst Package Surface

The Typst wrapper is an independent publication surface. The current candidate is `@preview/merman:0.3.0`, built from the prepared Merman `0.8.0-alpha.6` source line and Typst compiler `0.15.0`. It is not published by crates.io: `merman-typst-plugin@0.8.0-alpha.6` is the published Cargo transport crate, while `@preview/merman:0.3.0` is the user-facing Typst package containing the frozen wrapper, size-optimized WASM artifact, and third-party legal materials. Build provenance remains in the private artifact directory and is not part of the registry package.

Version `0.3.0` is the first Typst package rebuilt after the text-measurement closure reduction. ICU4X collation data and generated font-metric tables are no longer linked into the production artifact; the plugin keeps deterministic measurement and the existing ABI 2 exports. The package version changes because the shipped implementation closure and size characteristics are materially different, while the wrapper protocol remains compatible.

Before a Typst registry submission, bind the package to the reviewed 40-character source SHA and run the owner gates from `docs/release/RELEASING.md`: `verify-typst-profile-constants`, the `typst-wasm` dependency-closure check, the Typst size matrix, `build-typst-package --profile publish`, and the full Typst package smoke. Inspect the private artifact manifest under `target/typst-wasm-artifacts/` together with `LICENSE`, `THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES/` in the staged package. The provenance manifest is a preflight input, not a runtime package file. These are prepare/preflight checks only; manual Typst Universe submission requires explicit channel authorization. After submission, query the registry for the exact `0.3.0` package and update the package README and this file with observed publication evidence.

## Publish Order

Cargo metadata is the publish-order authority. Inspect the current dependency-safe projection with:

```bash
python3 tools/publish.py --list-crates-io-packages
```

The helper selects crates.io-publishable `workspace_members`, excludes names listed in
`workspace.metadata.merman-release.independent-packages`, follows every non-dev workspace path
dependency (including optional, target-specific, renamed, and build dependencies), rejects a
publishable crate that depends on a private workspace member, and topologically sorts the graph.
Independent package dependencies are treated as external registry inputs and are verified by their
dedicated workflows. Only crates within the same coupled batch use lexical ordering. The local
publish flow, release preflight, and release workflow consume this same projection; Markdown is
not parsed as a release-order database.

`roughr-merman` is versioned separately as `0.12.3`. The workflow reads each crate's own package
version, so it can skip already-published crates while still keeping one dependency-ordered list.

`tree-sitter-mermaid` `0.1.0` is a separately packaged language distribution. Its Cargo package is
`tree-sitter-mermaid`; its npm package is `@mermanjs/tree-sitter-mermaid`. Use
`release-tree-sitter-mermaid.yml`, not the generic independent-crate workflow. It builds native Node
prebuilds, verifies the root language WASM, installs the exact npm/Cargo/C candidate, stages a
grammar-subdirectory source archive and checksums, and publishes registry packages only when the
matching immutable `tree-sitter-mermaid-vX.Y.Z` tag passes the protected crates.io and npm
environments. GitHub Release publication is a separate explicit workflow input.
Because `merman-lsp` now consumes this crate for syntax highlighting, the exact Cargo version must
exist on crates.io before publishing a dependent workspace release. The scoped npm package is
independent of the workspace crates and supplies browser consumers with the same grammar WASM and
queries.

The initial npm package was bootstrapped manually from attested run `32114670734` with the
maintainer's 2FA-protected credential. Its registry tarball SHA-256 is
`a4e54b9caee7940cfbcffbe2b97d6edf04d8979b3eafe75fc0bba7804d04b23b`; it does not carry npm
provenance. Trusted Publishing is configured for later versions through
`release-tree-sitter-mermaid.yml` and the `npm` environment. The crates.io package was accepted by
run `32117231294` and has checksum
`34921c596d2732a74eb6489f1148df732163f0c4fd20737547396f40a588b559`. That run reported a false
failure because crates.io rejected curl's default User-Agent during post-publish download; direct
registry verification confirmed the published bytes. Do not rerun it to create a GitHub Release:
the independent native prebuild outputs differ across runs, so the later run's npm candidate is not
byte-identical to the bootstrapped artifact.

## Binding Release Chain

The binding-specific chain is:

```text
merman-analysis
  -> merman-editor-core
  -> merman-lsp
tree-sitter-mermaid
  -> merman-lsp

merman-render
  -> merman-export
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
  -> merman-wasm
```

This is why `merman-ffi` cannot fully package-verify until `merman-bindings-core` is published, and
`merman-bindings-core` cannot fully package-verify until a newer `merman-render` with `math`
is available on crates.io. `merman-wasm` comes last because it combines the browser wasm-bindgen
transport with the released binding core, renderer, ASCII, and editor-capable crates.

## Browser Package Group

The npm browser SDK is not a single Cargo publication and is intentionally outside the crates.io
topological order. After the selected source revision has passed release preflight, run
`release-web.yml` for the admitted package group: `@mermanjs/web`, `@mermanjs/web-analysis`,
`@mermanjs/web-editor`, `@mermanjs/web-ascii`, and `@mermanjs/web-render`. The workflow preflights
all existing versions and tags, then publishes missing exact versions directly under the requested
final tag in manifest order, with `@mermanjs/web` last. A retry skips matching published members.

The first version of a new split Web package cannot use npm Trusted Publishing before the package
exists. Run `release-web.yml` without publication, download the verified package-group artifact,
publish only the missing exact tarballs directly under the requested final tag with a maintainer's
2FA-protected npm credential, configure Trusted Publishing for those package names, then rerun the
workflow with publication enabled. Do not keep the bootstrap credential in GitHub Actions.

## Node npm Package Group

The experimental Node packages are one lockstep npm group: the native loader and five platform
packages (`@mermanjs/node`, `@mermanjs/node-darwin-arm64`, `@mermanjs/node-darwin-x64`,
`@mermanjs/node-linux-x64-gnu`, `@mermanjs/node-linux-x64-musl`, and
`@mermanjs/node-win32-x64-msvc`) plus the explicit Node-targeted WASM package
`@mermanjs/node-wasm`. Run `release-node.yml` against a reviewed immutable source commit after
the matching preflight succeeds. It builds and installs every native target and the WASM target,
preflights existing registry integrity and tags, then publishes missing exact versions directly
under the requested final tag in platform-first order, with the WASM package before the root
loader.

The first version of each npm package cannot use npm Trusted Publishing before the package exists.
For that one bootstrap, dispatch `release-node.yml` with `publish_to_npm=false` against the reviewed
immutable source and record its workflow run id. Download the verified
`merman-node-npm-package-group` artifact from that exact run, publish the five platform tarballs, the
WASM tarball, and then the loader directly under the requested final tag with a maintainer's
2FA-protected npm credential, and configure Trusted Publishing for all seven package names. Then
record that the bootstrap version remains without npm provenance; Trusted Publishing cannot add an
attestation to an existing tarball. From the next version onward, dispatch `release-node.yml` with
`publish_to_npm=true`; that run builds, verifies, and publishes its own same-run package group. If
its publish job fails, rerun that job within the same workflow run; a later run must build and verify
a new package group from the reviewed source. Do not keep an npm token in GitHub Actions.

The immutable `@mermanjs/node@0.8.0-alpha.5` loader tarball was packed before its package-local
changelog heading was dated, so the registry copy contains an `Unreleased` heading. This is a
documentation-only bootstrap defect: the source changelog is corrected, and the correction will
first appear in a later immutable package version.

## Pre-Publish Gates

Before publishing, run focused checks:

```bash
python3 tools/publish.py --list-crates-io-packages
cargo semver-checks check-release -p roughr-merman --color always
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo check -p merman-wasm
cargo nextest run -p merman-bindings-core -p merman-ffi -p merman-uniffi -p merman-wasm
```

The `roughr-merman` check uses the latest published compatible registry version as its baseline.
For later `0.12.x` patches, `0.12.3` is the established compatibility floor; the release workflows
pin `cargo-semver-checks` so the result does not depend on a maintainer's local tool version.

For crates.io packaging, prefer publish dry-runs once registry dependencies are available. The
release workflow packages every member of a topological batch first and records the exact `.crate`
digest. It then requires all missing members in that batch to pass this gate before the first real
publish attempt, so it also covers `merman-bindings-core`, `merman-ffi`, `merman-uniffi`, and
`merman-wasm`.

```bash
cargo publish -p merman-render --locked --dry-run --registry crates-io
cargo publish -p merman-export --locked --dry-run --registry crates-io
cargo publish -p merman-bindings-core --locked --dry-run --registry crates-io
cargo publish -p merman-ffi --locked --dry-run --registry crates-io
cargo publish -p merman-uniffi --locked --dry-run --registry crates-io
cargo publish -p merman-wasm --locked --dry-run --registry crates-io
```

Before upstream crates for the same release are visible in crates.io, keep using `cargo package
--list` only as a file-list check. It does not replace publish dry-run verification.

The credentialed workflow writes `batch-NNN-prepared.json` before each batch and
`batch-NNN-result.json` after registry reconciliation. Both conform to
`distribution/crates-io/receipt-schema-v1.json` and bind the source commit/tree, Cargo and Rust
toolchains, publish graph, manifest bytes, `.crate` bytes, and observed registry checksum. A batch
result of `pending_recovery` or `mismatch` blocks every dependent batch. Recovery re-packages the
same source and skips only exact checksum matches. A rerun consumes the prior attempt artifact; a
new manual recovery uses the same immutable release tag and supplies its prior workflow id through
`recovery_run_id`. Source, tree,
toolchain, publish plan, manifest, and `.crate` identity must all agree before recovery continues,
and the publisher never yanks automatically.

## Pre-Alpha.3 Package Evidence Snapshot

The table below is historical pre-publication evidence captured on 2026-07-03. Query crates.io or
the owning artifact release directly for current availability; do not infer it from this snapshot.

| Crate | Gate | Current result |
| --- | --- | --- |
| `dugong-graphlib` | crates.io lookup | Published |
| `manatee` | crates.io lookup | Published |
| `merman-core` | crates.io lookup | Published |
| `merman-elk-layered` | crates.io lookup | Published |
| `roughr-merman` | release workflow skip-if-published check | Versioned separately as `0.12.1`; publish only when that crate version is not already visible |
| `dugong` | crates.io lookup | Published |
| `merman-analysis` | release workflow dry-run before publish | Pending after `merman-core` is published |
| `merman-ascii` | `cargo publish -p merman-ascii --locked --dry-run --allow-dirty --registry crates-io` | Pass locally; not yet published |
| `merman-layout-elk` | crates.io lookup | Published |
| `merman-editor-core` | release workflow dry-run before publish | Pending after `merman-analysis` is published |
| `merman-render` | `cargo publish -p merman-render --locked --dry-run --allow-dirty --registry crates-io` | Pass locally after release-source fix; not yet published |
| `merman-export` | release workflow dry-run before publish | Pending after `merman-render` is published |
| `merman` | release workflow dry-run before publish | Pending after `merman-export` is published |
| `merman-lsp` | release workflow dry-run before publish | Pending after `merman-editor-core` and `merman` are published |
| `merman-bindings-core` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-cli` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-rustdoc` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-ffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-typst-plugin` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-uniffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-wasm` | release workflow dry-run before publish | Pending after `merman-bindings-core`, `merman-editor-core`, and `merman` are published |

## Publish Guardrail

Do not run `cargo publish` as part of an implementation lane unless the release operator explicitly
requests it. `tools/publish.py` computes the current order, and `release-crates.yml` is the normal
caller of `scripts/crates_io_release.py publish-receipted`. Do not invoke that command locally merely
to test it; focused unit tests own response-loss, checksum-mismatch, and partial-recovery behavior.

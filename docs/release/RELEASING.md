# Releasing

Status: maintained release operator guide.

Merman releases use a preflight-first flow. Run the release preflight workflow against the intended
source ref and version before any registry or GitHub Release publication. After preflight passes,
push a `v<version>` workspace tag only for the tag-triggered CLI/LSP and crates.io surfaces. Flutter
uses its own `flutter-v<version>` tag because pub.dev requires a tag-triggered workflow; Python,
Android, Apple, Web, Node, and VS Code remain owner-dispatched surfaces.

Release classification follows SemVer syntax, not the major version number. Versions without a
prerelease suffix, including `0.7.0` and `0.8.0`, are regular releases. Only versions with a
prerelease component such as `0.8.0-alpha.5`, `0.8.0-beta.1`, or `0.8.0-rc.1` use a prerelease
channel. Do not describe the project or a `0.x` release as alpha solely because its major version
is zero.

## Release Workflows

| Workflow | Publishes | Channel |
| --- | --- | --- |
| `release-preflight.yml` | Nothing; dry-run/build verification only | GitHub Actions artifacts |
| `release.yml` | `merman-cli` and `merman-lsp` binary archives, installers, and checksums | GitHub Release |
| `release-crates.yml` | Rust workspace crates | crates.io |
| `release-yank-crates.yml` | Selected workspace-coupled versions after an incomplete publication | crates.io |
| `release-apple.yml` | `Merman.xcframework-<tag>.zip` and checksum | GitHub Release artifact upload |
| `release-python.yml` | `merman` wheels for Linux, macOS, and Windows | GitHub Release + PyPI |
| `release-flutter.yml` | `merman` with Native Assets for Android, iOS, macOS, Windows, and Linux | pub.dev |
| `release-android.yml` | `merman-android-<tag>.aar` | GitHub Release |
| `release-web.yml` | admitted `@mermanjs/web` browser package group | npm |
| `release-node.yml` | experimental `@mermanjs/node` loader, five native platform packages, and explicit `@mermanjs/node-wasm` | npm |
| `release-tree-sitter-mermaid.yml` | `tree-sitter-mermaid` crate, `@mermanjs/tree-sitter-mermaid`, language WASM, and source archive | crates.io, npm, and GitHub Release |
| `vscode-extension.yml` | Platform-specific `merman-vscode` VSIX artifacts | GitHub Actions artifacts |
| `homebrew.yml` | Nothing; Homebrew/core formula health check only | Homebrew |

The word "FFI" covers two different deliverables. `merman-bindings-core`, `merman-ffi`,
`merman-uniffi`, and `merman-wasm` are source crates in the crates.io graph. Android AAR, Apple
XCFramework, Python wheels, and Flutter Native Assets are separately built artifacts owned by their
own workflows. `merman-android-jni` is intentionally `publish = false`; its public delivery is the
AAR, not a crates.io package. A green preflight proves that those owner builds can succeed, but it
does not publish them and it does not imply that every surface ran.

Run the static, machine-readable FFI/native ownership check before dispatching any owner workflow:

```bash
VERSION="<version>"
python3 scripts/release_surface_contract.py --version "$VERSION"
```

The check is a contract of workflow, artifact profile, and source-crate ownership for the five
FFI/native surfaces above. It does not replace the complete workflow table, and it does not cache
registry status; after publication, query each owning registry or GitHub Release directly.

Most platform publish workflows are manual `workflow_dispatch` workflows. Python, Android, and
Apple accept a `release_tag`, resolve the matching immutable tag commit and tree, and build all
artifacts from that source. The workflow definition may be dispatched from `main`, but `main` is
never an artifact source for those releases. Web, Node, and VS Code expose an explicit `source_ref`
for owner-specific prerelease or recovery flows; their workflows resolve it to an immutable commit
before building. Flutter is the exception: pub.dev automated publishing only accepts GitHub
Actions runs triggered by a pushed git tag, so `release-flutter.yml` publishes from a
`flutter-v<version>` tag and uses manual runs with an explicit `source_ref` for validation only.
The pub.dev trusted publisher must use the matching `flutter-v{{version}}` tag pattern. The
crates.io workflow is idempotent only when deterministic local re-packaging produces the checksum
already recorded by the registry. Before each topological batch it packages every member from the
unchanged source, writes a source/tool/artifact receipt, and preflights existing registry checksums. Every missing
member in the batch must pass its locked publish dry-run before the first member is published; each
then receives one publish attempt. The workflow does not enter the next
batch until every checksum in the current batch matches; delayed visibility or a lost response
produces a durable pending-recovery receipt instead of a blind retry.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io publish | `CARGO_REGISTRY_TOKEN` secret in the `crates.io` environment |
| crates.io incident yank | Dedicated `CARGO_REGISTRY_YANK_TOKEN` secret with yank permission in the `crates.io` environment |
| pub.dev | Trusted Publishing / OIDC configured for `merman`, this repository, `release-flutter.yml`, and `flutter-v{{version}}` |
| PyPI | Trusted Publishing / OIDC configured for `merman` and `release-python.yml` |
| npm | Trusted Publishing / OIDC configured for every admitted `@mermanjs/web*` and `@mermanjs/node*` package plus `@mermanjs/tree-sitter-mermaid`, this repository, each owning release workflow, and the `npm` environment |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |
| VS Code Marketplace | Not configured. Marketplace publishing would need `VSCE_PAT`, an explicit publish job, and VSIX provenance verification before enabling. |

Publish jobs use GitHub Environments (`crates.io`, `pypi`, `pub.dev`, `npm`, and `github-release`).
Configure required reviewers on those environments if publication should require explicit approval.

If a crates.io run stops after publishing only part of a release, keep the tag immutable and retain
the uploaded `crates-io-receipts-*` artifact. Rerun the workflow only from that same immutable tag.
A GitHub rerun downloads the prior attempt's receipts; a new manual recovery must pass
the prior workflow id as `recovery_run_id`. The publisher recreates every `.crate` and requires the
prior prepared/result receipts to match the source, tree, toolchain, graph, manifests, and artifacts
before it observes or mutates the registry. Matching versions are skipped, missing versions continue
in topological order, and a different registry checksum stops before further publication. A mismatch
requires an explicit maintainer decision; the normal publisher never yanks.

For a new-run crates.io recovery, use the immutable tag and the run id that uploaded the prior
receipts:

```bash
gh workflow run release-crates.yml \
  -f release_tag="$RELEASE_TAG" \
  -f source_ref="refs/tags/$RELEASE_TAG" \
  -f recovery_run_id="<prior-run-id>"
```

Android Maven Central publishing is credential-blocked. Android now declares Maven publication
metadata, but Central Portal credentials, signing secrets, and a dedicated publish job still need to
be configured.

VS Code Marketplace publishing is credential-blocked. `.github/workflows/vscode-extension.yml`
packages and verifies platform VSIX artifacts only; Marketplace publication needs a dedicated
publish job, `VSCE_PAT`, and artifact provenance verification before it is enabled.

The PyPI project `merman` exists. Keep PyPI Trusted Publishing configured for owner `Latias94`,
repository `merman`, workflow `release-python.yml`, and environment `pypi`. A PyPI Pending
Publisher is only needed before the first trusted publish of a new project name.

The public browser packages are `@mermanjs/web`, `@mermanjs/web-analysis`,
`@mermanjs/web-editor`, `@mermanjs/web-ascii`, and `@mermanjs/web-render`. Configure npm Trusted
Publishing for every package in that lockstep group with workflow file `release-web.yml` and GitHub
environment `npm`. Trusted publishes automatically include npm provenance; the workflow does not
need `--provenance`. For any package name that does not yet exist, first run the workflow without
publication, publish only the verified missing tarball directly under the requested final tag with a
maintainer's 2FA-protected credential, configure its trusted publisher, and rerun the workflow with
publication enabled. Do not add the bootstrap credential to GitHub Actions.

The independent syntax package is `@mermanjs/tree-sitter-mermaid`, owned by
`release-tree-sitter-mermaid.yml`. Its first `0.1.0` npm publication used a verified workflow
artifact and a maintainer's 2FA-protected bootstrap credential before Trusted Publishing could be
configured for that exact scoped name. That manual bootstrap does not carry npm provenance; keep
that boundary explicit. Trusted Publishing is now configured for later versions through
`release-tree-sitter-mermaid.yml` and the `npm` environment. The Rust crate remains
`tree-sitter-mermaid`; the two registry packages share a version but not a registry name.

Native prebuild bytes are not assumed to reproduce across independent workflow runs. A first-publish
or recovery operator must publish and reconcile the exact candidate from one workflow run, then
rerun only that run's failed jobs. Do not manually publish a candidate from one run and ask a later
run to accept a separately rebuilt npm tarball as byte-identical.

The experimental Node group is `@mermanjs/node`, its five native platform packages, and the
explicit `@mermanjs/node-wasm` package. Its workflow builds and install-smokes each actual native
target plus the Node WASM artifact, packages platform binaries and the WASM package before the root
loader, and uses the same direct-publish plus integrity-preflight boundary as the browser group.
npm only allows a trusted publisher to be configured for an existing package, so the first release
of each new Node package requires the documented one-time, 2FA-protected bootstrap from the
verified `release-node.yml` workflow artifact; configure OIDC for all seven names immediately
afterward. The bootstrap artifact is for that one manual publication only, and its registry version
remains without npm provenance; a later OIDC run cannot add provenance to an existing tarball. From
the next version onward, start a publishing workflow run against the reviewed source so it builds,
verifies, and publishes its own same-run package group. If publication fails, rerun the failed job
in that workflow run rather than asking a later run to trust an older artifact. Do not add a
persistent npm token to the repository workflow.

For an npm-only alpha test, `release-node.yml` treats `release_tag` as the package version label and
`source_ref` as the build source. The source may be a reviewed full commit SHA newer than the
same-named workspace tag; the workflow records the resolved commit in the package-group manifest,
and release notes must not claim that separately published channels are byte-identical.

The npm publish job is intentionally narrow: it runs on GitHub-hosted Ubuntu with Node 24, enters
the `npm` environment, requests `id-token: write`, and checks out the trusted workflow revision plus
the immutable source commit without credentials. The source checkout supplies only the package
surface descriptor; the trusted revision verifies the downloaded package-group hashes before
publishing missing packages directly under the final tag. The job must not build, test, or execute
source scripts or scripts contained in the downloaded artifact. Do not add `NPM_TOKEN`,
`NODE_AUTH_TOKEN`, `--provenance`, `provenance=false`, or `NPM_CONFIG_PROVENANCE=false`.

The Apple workflow currently publishes a zipped `Merman.xcframework` and checksum as GitHub Release
assets. It does not yet make the repository directly consumable as a remote SwiftPM package with a
`.binaryTarget(url:checksum:)`, because that checksum must be known and committed before the release
tag. Treat direct remote SwiftPM support as a separate release-manifest design task.

Homebrew installs `merman-cli` from the formula in `homebrew/core`; it is not published directly by
this repository. After a stable release, Homebrew's autobump flow should pick up the new GitHub tag.
Use the scheduled `homebrew.yml` workflow, or dispatch it with an optional `expected_version`, to
verify formula freshness and run the installed CLI, linkage, and formula-test contracts. The
workflow is deliberately independent of repository tags because Homebrew/core tracks stable
versions on its own publication schedule.

## Release Evidence

Before tagging, identify the packages and artifact workflows that will actually publish, then run
their owner-specific dry runs. `PACKAGE_SURFACES.md` is a package-choice guide; manifests,
descriptors, artifact profiles, and workflows are the executable evidence. After publication, query
the owning registry or `gh release view "v$VERSION"` directly instead of inferring availability from
a repository-maintained status cache.

## Version Checklist

`Cargo.toml` `[workspace.package].version` is the sole authority for a workspace release. While no
next version is selected, keep the root changelog at `[Unreleased]` and leave the workspace version
unchanged. After the maintainer selects a version, prepare the projection in an exclusive Git
linked worktree. The `set` command rejects the primary checkout, tracked or untracked dirt, and an
npm executable that does not match the `packageManager` pin in `playground/package.json`. Cargo and
npm owners prepare manifests and locks in a disposable detached worktree; the coordinator validates
the complete projected tree and source preimage, checks one binary patch, then applies that patch
once to the caller worktree. Verify every checked-in path without supplying a second version value:

```bash
python3 scripts/release-version.py set --version <version>
python3 scripts/release-version.py
```

Preparation or pre-apply validation failures leave the caller worktree unchanged. Inspect the
reported owner failure, keep the release worktree clean, and rerun the same command; do not copy
partial files out of the disposable worktree or hand-edit generated locks.

The gate discovers workspace members and validates their inherited package versions, internal workspace dependency requirements, `Cargo.lock`, Web package and lock metadata, the Playground's local Web lock, the fuzz-workspace lock, Python's PEP 440 projection, and Android and Flutter manifests. For a prerelease, every coupled root workspace dependency must use the exact Cargo requirement `=X.Y.Z-alpha.N`, `=X.Y.Z-beta.N`, or `=X.Y.Z-rc.N`; stable releases continue to use the ordinary compatible requirement. This permits intentional alpha API breaks without allowing a fresh Cargo resolution to mix sibling release lines.

`cargo check --locked` is not sufficient evidence for a prerelease. The release gates also create
fresh consumers without a copied lockfile and compile the candidate graph plus the previous facade
against candidate sibling packages when both versions are on the same Cargo compatibility line. A
failure in that same-line previous-facade lane means the release must restore compatibility or start
a new release line; do not rely on downstream lockfiles to hide the mixed graph. Published registry
tarballs are immutable, so a dependency requirement defect cannot be repaired by editing this
repository after publication; the next release must carry the corrected manifest and pass this gate.

The prerelease compatibility gate is admission control for a new version. A backfill of an
immutable prerelease that was published before this gate existed may use the original tag and the
owner-specific artifact workflows while recording its historical compatibility exception. It must
not move the tag or treat the exception as permission for a later prerelease.

Keep the target Changelog entry marked `Unreleased` during ordinary preparation. Use an unversioned `[Unreleased]` heading while the next workspace version is undecided, then add the selected version before release preflight. Immediately before the immutable preflight, replace `Unreleased` with the intended tag date in `YYYY-MM-DD` form and verify that its version matches the workspace release authority. Do not tag an `Unreleased` entry or reuse a date from an abandoned release attempt.

Treat the root `CHANGELOG.md` as the canonical project-wide release narrative and package changelogs as audience-specific projections of the same release delta. Update only the package changelogs for surfaces included in the release; do not copy the complete root entry or create one changelog per Rust crate.

| Surface | Registry or audience behavior | Changelog source |
| --- | --- | --- |
| Node | The root loader tarball includes the user-facing changelog for the seven-package group | `platforms/node/CHANGELOG.md` |
| Flutter/Dart | pub.dev renders the package-root changelog as its Changelog tab | `platforms/flutter/CHANGELOG.md` |
| Python | PyPI project metadata links Python users to the package changelog | `platforms/python/merman/CHANGELOG.md` |
| Android | The Android package README links consumers to its JNI/AAR-specific history | `platforms/android/CHANGELOG.md` |
| Apple | The Apple package README links consumers to its Swift/XCFramework-specific history | `platforms/apple/CHANGELOG.md` |
| VS Code | The unpublished extension has an independent version and release boundary | `tools/vscode-extension/CHANGELOG.md` |

For workspace-coupled packages, keep the target package entry at `Unreleased` during preparation and stamp it with the same intended tag date as the root entry immediately before immutable preflight. For an authorized channel-only publication, keep the root entry at `Unreleased` but stamp the channel's package-local entry before building its immutable package artifact. Each projection should contain only user-visible behavior, migrations, compatibility notes, and performance claims verified for that surface. Independently versioned surfaces keep their own version and publication date.

README files are ordinary source documentation and `release-version.py` never rewrites them. Before immutable preflight, review the root README, every package README shipped by an authorized surface, and the closest installation guides. Release-facing Cargo dependency examples should use the exact target prerelease version instead of a moving default-branch Git dependency. Source-only Git commands must be labeled as source installs and pin a reviewed full commit. Remove stale statements that an already-published version is unavailable, old published baselines, and future tense such as “after this version is published”. Historical reports may retain historical wording. Confirm publication through the owning package registry or artifact workflow before recommending a released install command, and repeat this documentation pass after any partial-release recovery.

A focused audit command is:

```bash
rg -n --glob '**/README.md' --glob '**/CHANGELOG.md' \
  '0\.[0-9]+\.[0-9]+-(alpha|beta|rc)\.[0-9]+|\[[^]]+\] - Unreleased|published registry packages can still|candidate from Git|[Aa]fter .*is published|[Ww]hen .*is published|git\s*=\s*"https://github\.com/Latias94/merman' \
  README.md CHANGELOG.md crates distribution docs platforms tools
```

Classify each match rather than applying a repository-wide mechanical rewrite. Package-local READMEs included in crates, cargo-dist archives, npm tarballs, wheels, Flutter packages, Apple/Android bundles, Typst packages, or VSIX files are part of the release experience even though they are not version authorities.

After publication or recovery, query each owning registry and GitHub Release independently, then
repeat the README audit and update `PUBLISH_ORDER.md` from candidate state to the published
baseline. Do not imply that npm, PyPI, pub.dev, Android, Apple, Typst, crates.io, and GitHub binaries
move in lockstep. The release is not operationally complete while a current install command names
an older version or current prose still describes an externally published target as unavailable;
commit the documentation reconciliation or report it as explicit unfinished release work.

For Web and Node npm groups, verify every package member's exact registry integrity and complete
dist-tag map against the downloaded package-group manifest. npm Trusted Publisher can publish under
the requested final tag but cannot repair tags afterward, so existing integrity or tag conflicts
must fail before any mutation. Confirm that the published tarball already contains the dated
package-local changelog; if it does not, record the immutable artifact defect and fix the source for
the next version. A channel-only npm release does not select the next workspace version.

The unpublished VS Code extension, the Typst package wrapper, and `roughr-merman` own independent
version tracks. They are intentionally excluded from workspace projection. Record the workspace
runtime bundled in VSIX and Typst artifacts through provenance instead of rewriting those package
versions. Before any later `roughr-merman 0.12.x` publication, run `cargo-semver-checks` against
the latest published compatible baseline. Version `0.12.3` is the forward-looking compatibility
floor; the historical `0.12.1` to `0.12.3` recovery is not re-litigated by future release gates.

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Release Preflight

Before tagging or publishing, run:

```bash
VERSION="<version>"
SOURCE_SHA="$(git rev-parse HEAD)"
gh workflow run release-preflight.yml -f version="$VERSION" -f source_ref="$SOURCE_SHA"
```

The preflight workflow verifies release versions, the static release-surface contract,
prerelease fresh-resolution compatibility, independent-crate Rust API compatibility,
package file lists, registry-independent Rust crate publish dry-runs, Python wheels, Android AAR
builds, Apple XCFramework builds, the web npm package dry-run, Node native package-group
build/install smokes, platform VSIX packaging, and Flutter `dart pub publish --dry-run`. It does
not publish to any registry.

Release CI keeps integrity checks at boundaries where bytes or trust cross jobs or registries. GitHub
Actions are pinned to immutable commit references; source identity uses the commit and tree; SHA-256
is used for downloaded release tools, staged release artifacts, and immutable registry reconciliation
(crates.io, npm, and PyPI). Ordinary unit tests and in-workspace builds do not calculate or compare
extra hashes. pub.dev uses a member-level content comparison because Dart rewrites tar metadata.
Platform GitHub Release asset uploads fail closed on an existing name, and Tree-sitter native
prebuild recovery stays within the same workflow run.

Release-archive smoke tests should verify user-observable contracts rather than incidental representation choices. Accept legal binary token and whitespace forms, and allow valid asynchronous notification ordering while still requiring bounded output, the expected protocol responses, successful exit, and exact archive contents. Reproduce failures against the final archive before changing product code.

Record `VERSION` and `SOURCE_SHA` with the preflight run. The run must be green for that exact immutable commit, not merely for a branch name that can move while preflight is running. Create the tag only through the `Tag And Push` step after that run succeeds. A complete native release still requires explicit, successful owner workflow runs for every authorized artifact surface; absence of an owner run is incomplete delivery, not an implicit skip.

For local spot checks, run the normal Rust and platform gates:

```bash
cargo nextest run --cargo-quiet
python3 scripts/artifact_profile_recipe.py cli-release --build-host --locked
python3 -m py_compile \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/build-native.py
bash -n scripts/build-apple-xcframework.sh platforms/ios/build-ios.sh
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

On macOS with Xcode:

```bash
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

For Flutter:

```bash
python3 platforms/flutter/build-native.py host
cd platforms/flutter
flutter pub get
flutter analyze
dart format --set-exit-if-changed \
  lib/merman.dart lib/src/merman_ffi.dart lib/src/operation_metadata.dart \
  example tool hook
dart run tool/abi3_contract_test.dart
dart run example/main.dart
dart pub publish --dry-run
```

The host smoke is the ordinary local gate. The release and preflight workflows additionally build
all Android, iOS, macOS, Windows, and Linux Native Assets before the pub dry run.

For local npm validation:

```bash
cd platforms/web
npm ci
npm run build
npm run smoke
npm run verify:packages
artifact_dir="$(mktemp -d)"
python3 ../../scripts/web_package_group.py pack \
  --root ../.. \
  --descriptor web-surface-descriptor.json \
  --artifact-dir "$artifact_dir" \
  --version "<version>" \
  --source-sha "$(git -C ../.. rev-parse HEAD)" \
  --target-dist-tag alpha
python3 ../../scripts/web_package_group.py verify-artifact \
  --manifest "$artifact_dir/web-package-group.json" \
  --artifact-dir "$artifact_dir" \
  --descriptor web-surface-descriptor.json
rm -rf "$artifact_dir"
```

Normal Web releases must use `release-web.yml`. It preflights every existing exact package version
and target tag, then publishes missing versions directly under the requested `alpha`, `beta`, `rc`,
or `latest` tag in manifest order, with the default package last. Do not publish a member manually:
the workflow's manifest and post-publish integrity checks are the source of truth for a retry.

If a Web publication is partial, do not start a new build for the retry. Dispatch the same workflow
with `recovery_run_id` set to the original `release-web.yml` run and `source_ref` set to that run's
verified package-group manifest `source_sha`:

```bash
gh workflow run release-web.yml \
  -f release_tag="v<version>" \
  -f source_ref="<manifest-source-sha>" \
  -f recovery_run_id="<original-web-run-id>" \
  -f publish_to_npm=true
```

The recovery path downloads the original unexpired artifact, verifies its source, version, tag,
and tarball integrity, and skips members already accepted by npm. A new build from the same source
is not an equivalent recovery artifact and can be rejected if npm already accepted one member.

For local Node package validation on the current host:

```bash
cd platforms/node
npm ci
npm test
npm run check:packages
package_root="$(mktemp -d)"
node scripts/assemble-packages.mjs --loader-only --output-root "$package_root"
npm pack "$package_root/node" --dry-run
```

The release preflight and `release-node.yml` remain responsible for building, installing, and
render-smoke-testing all five native targets plus the explicit Node WASM package. A local host build
does not substitute for that matrix. Normal Node releases publish the five platform packages and
the WASM package before the root loader and verify the exact package-group integrity and target tags
before publishing any missing package directly under the public prerelease tag.

For local VS Code VSIX validation:

```bash
python3 scripts/artifact_profile_recipe.py lsp-stdio-release --build-host --locked
python3 scripts/artifact_profile_recipe.py cli-release --build-host --locked
cd tools/vscode-extension
npm ci
npm test
npm run prepare:binaries
target="$(node -p 'process.platform + "-" + process.arch')"
npm run package -- --target "$target" --out "merman-vscode-${target}.vsix"
npm run verify:vsix -- --vsix "merman-vscode-${target}.vsix" --platform "$target" --target "$target"
```

The extension manifest is the authority for the VSIX version. The workspace version identifies the
bundled `merman-lsp` and `merman-cli` runtime for provenance only; it must not rewrite or validate
the extension's independent version. Keep the changelog under `Unreleased` until the first `0.1.0`
Marketplace publication is intentionally prepared.

Before changing Web or Typst artifact profiles, also run the surface-specific
gates:

```bash
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
cargo run -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- verify-typst-profile-constants
python3 scripts/verify_artifact_dependency_closures.py --profile typst-wasm
cargo run --locked -p xtask -- build-typst-package --profile publish
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

The Web package build uses `wasm-pack --profile wasm-size`, so CI and local release machines need
`wasm-pack` 0.15.0 or newer. `npm run verify:packages --prefix platforms/web` validates each descriptor
package as an independently installable artifact. Release preflight must build/check each admitted
package's matching TypeScript, wasm-bindgen glue, single WASM, provenance, generated binding API,
editor schema,
and complete 35-family language catalog; it is not valid to publish only a wrapper declaration.
Typst package builds additionally require Binaryen `wasm-opt version 131` and `wasm-tools`; the
post-link optimizer and stripper versions are recorded in the artifact manifest.
`merman-typst-plugin` is the Typst-compatible transport and must remain separate from
browser/wasm-bindgen artifacts. Cargo defaults are intentionally empty. The sole public package
profile is `publish`; it consumes the exact `typst-wasm` artifact recipe, which pins
`default-features = false`, the `svg`, `analysis`,
`layout-cytoscape`, and `layout-elk` features, the `wasm-size` Cargo profile, and the
`wasm32-unknown-unknown` target. Feature bundles are additive closure descriptions, not alternate
release artifacts. The exact dependency gate admits `json5`, `lol_html`, and `url` as measured
pure-Rust dependencies of invariant Mermaid language, configuration, and sanitization semantics.
They remain covered by the exact artifact size budget and final WASM import gate; browser bindings,
randomness, clocks, and other system adapters remain forbidden. Release validation requires Typst
plugin ABI 2, independently from native ABI 3,
the closed export surface including
`analyze_json`, and the descriptor-owned `publish` artifact. Its private directory contains the
stripped WASM and provenance manifest; `--skip-wasm-build` is allowed only because it validates the manifest's
exact artifact profile, package feature bundle, default-feature policy, inputs, tools, versions,
flags, and artifact digest before package
reuse. Do not package the private raw Cargo output under `target/wasm-build/`. The artifact provenance
manifest remains private to `target/typst-wasm-artifacts/`; the package transaction binds the
verified artifact to an in-memory frozen wrapper/license source snapshot and must fail before
replacing the prior version if live source or any staged byte changes.

These commands are Typst owner preflight only. The Cargo crate `merman-typst-plugin` and the Typst Universe package `@preview/merman:0.3.0` are separate publication surfaces; publishing the crate does not publish the wrapper. The 0.3.0 wrapper was published on 2026-09-01 after the exact alpha.6 source candidate passed its owner gates. For a later version, retain the generated package and private artifact receipt, verify the registry's exact package version after acceptance, and only then change current installation guidance from a local `--package-path` candidate to the registry package.

## Tag And Push

```bash
test -n "$VERSION"
test -n "$SOURCE_SHA"
test "$(git rev-parse HEAD)" = "$SOURCE_SHA"
git tag "v$VERSION" "$SOURCE_SHA"
git push origin "v$VERSION"
```

Do not move or force-update release tags after publication. Release tags are the immutable source
anchor for crates, CLI/LSP artifacts, and platform assets.

`release.yml` creates the primary GitHub Release and uploads CLI and standalone LSP artifacts.
Platform workflows
upload additional assets to that existing release when it is present; otherwise they leave GitHub
Actions artifacts for manual attachment.
Asset uploads are intentionally non-destructive: an existing asset name is not overwritten. A rerun
that encounters an existing platform asset fails closed and should be resolved by inspecting the
release, rather than downloading and comparing every asset in CI.

After the primary release exists, run platform publish workflows manually:

```bash
RELEASE_TAG="v<version>"
gh workflow run release-python.yml -f release_tag="$RELEASE_TAG" -f publish_to_pypi=true
gh workflow run release-android.yml -f release_tag="$RELEASE_TAG"
gh workflow run release-apple.yml -f release_tag="$RELEASE_TAG"
gh workflow run release-web.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish_to_npm=true
gh workflow run release-node.yml -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" -f publish_to_npm=true
gh workflow run vscode-extension.yml -f source_ref="$RELEASE_TAG"
gh workflow run homebrew.yml
```

The VS Code workflow currently packages and verifies platform VSIX artifacts only; Marketplace or
Open VSX publishing requires a separate credential-backed release workflow.

Do not rely on a manual `release-flutter.yml` run for pub.dev publication. A manual run still builds,
bundles native artifacts, analyzes, formats, and performs `dart pub publish --dry-run`, but the real
`dart pub publish --force` step only runs from the pushed `flutter-v<version>` tag. For an
independent Flutter release, first validate the intended source, then create the package tag on the
resolved reviewed commit:

```bash
gh workflow run release-flutter.yml \
  -f release_tag="flutter-v<version>" \
  -f source_ref="<reviewed-commit>"
git tag "flutter-v<version>" "<reviewed-commit>"
git push origin "flutter-v<version>"
```

The Flutter build job emits a package archive receipt binding the release commit, tree, version, and
archive digest. The pub.dev job runs the trusted verifier from the workflow revision, enforces
bounded regular-file-only extraction into a new directory, and configures Dart OIDC only after that
verification succeeds. Do not replace this boundary with a direct `tar -x` step.

For a workflow-only recovery after a release tag already exists, Python, Android, and Apple may use
the updated workflow definition from `main`, but they still check out and verify the immutable
release-tag commit and tree. Credentialed publication workflows that expose `source_ref` require
the matching immutable tag/ref or a reviewed 40-character commit; a mutable `main` ref is valid only
for an explicitly non-publishing build or validation run.

## Follow-On Registry Work

- Add Android Maven Central publishing after Central Portal credentials and signing secrets are configured.
- Add device-level Flutter smoke coverage after a stable CI target is chosen for each platform.

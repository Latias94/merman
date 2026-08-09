# Releasing

Status: maintained release operator guide.

Merman releases use a preflight-first flow. Run the release preflight workflow against the intended
source ref and version before any registry or GitHub Release publication. After preflight passes,
push a `v*` tag whose version matches every package manifest that will publish in that release.

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
| `release-flutter.yml` | `merman` with injected Android, iOS, macOS, Windows, and Linux native artifacts | pub.dev |
| `release-android.yml` | `merman-android-<tag>.aar` | GitHub Release |
| `release-web.yml` | admitted `@mermanjs/web` browser package group | npm |
| `vscode-extension.yml` | Platform-specific `merman-vscode` VSIX artifacts | GitHub Actions artifacts |
| `homebrew.yml` | Nothing; Homebrew/core formula health check only | Homebrew |

Most platform publish workflows are manual `workflow_dispatch` workflows. Python, Android, and
Apple accept a `release_tag`, resolve the matching immutable tag commit and tree, and build all
artifacts from that source. The workflow definition may be dispatched from `main`, but `main` is
never an artifact source for those releases. Web and VS Code still expose an explicit `source_ref`
for their owner-specific recovery flows. Flutter is the exception: pub.dev automated publishing
only accepts GitHub Actions runs triggered by a pushed git tag, so `release-flutter.yml` publishes
from the `v*` tag push and uses manual runs for validation only. The crates.io workflow is
idempotent for already-published crate versions, so a rerun can continue after a partial publish
caused by registry propagation delays. For unpublished crates, it performs
`cargo publish --dry-run --locked` immediately before the real publish, after upstream workspace
dependencies in the same release have become visible in crates.io.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io publish | `CARGO_REGISTRY_TOKEN` secret in the `crates.io` environment |
| crates.io incident yank | Dedicated `CARGO_REGISTRY_YANK_TOKEN` secret with yank permission in the `crates.io` environment |
| pub.dev | Trusted Publishing / OIDC configured for `merman`, this repository, `release-flutter.yml`, and the release tag pattern |
| PyPI | Trusted Publishing / OIDC configured for `merman` and `release-python.yml` |
| npm | Trusted Publishing / OIDC configured for every admitted `@mermanjs/web*` package, this repository, `release-web.yml`, and the `npm` environment |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |
| VS Code Marketplace | Not configured. Marketplace publishing would need `VSCE_PAT`, an explicit publish job, and VSIX provenance verification before enabling. |

Publish jobs use GitHub Environments (`crates.io`, `pypi`, `pub.dev`, `npm`, and `github-release`).
Configure required reviewers on those environments if publication should require explicit approval.

If a crates.io run stops after publishing only part of a release, keep the failed tag immutable and
publish the corrected dependency graph under a new version. Use `release-yank-crates.yml` only for
the exact workspace-coupled crate names that became visible from the failed tag. The workflow
validates the tag, package set, and package versions before it receives the dedicated yank token;
do not broaden the normal publish token merely to support incident rollback.

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
need `--provenance`.

The npm publish job is intentionally narrow: it runs on GitHub-hosted Ubuntu with Node 24, enters
the `npm` environment, requests `id-token: write`, checks out only `github.workflow_sha` without
credentials, downloads the verified package-group data artifact, verifies its hashes against the
trusted descriptor, then reconciles the group. It must not checkout the dispatch `source_ref`,
build, test, or execute a script contained in the downloaded artifact. Do not add `NPM_TOKEN`,
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

`Cargo.toml` `[workspace.package].version` is the sole authority for a workspace release. Prepare
the projection in an exclusive Git worktree, then verify every checked-in path without supplying a
second version value:

```bash
python3 scripts/release-version.py set --version <version>
python3 scripts/release-version.py
```

The gate discovers workspace members and validates their inherited package versions, internal workspace dependency requirements, `Cargo.lock`, Web package and lock metadata, the Playground's local Web lock, the fuzz-workspace lock, Python's PEP 440 projection, Android and Flutter manifests, CocoaPods metadata, and iOS framework bundle versions.

Keep the target Changelog entry marked `Unreleased` during ordinary preparation. Immediately before the immutable preflight, replace it with the intended tag date in `YYYY-MM-DD` form and verify that its version matches the workspace release authority. Do not tag an `Unreleased` entry or reuse a date from an abandoned release attempt.

Treat the root `CHANGELOG.md` as the canonical project-wide release narrative and package changelogs as audience-specific projections of the same release delta. Update only the package changelogs for surfaces included in the release; do not copy the complete root entry or create one changelog per Rust crate.

| Surface | Registry or audience behavior | Changelog source |
| --- | --- | --- |
| Flutter/Dart | pub.dev renders the package-root changelog as its Changelog tab | `platforms/flutter/CHANGELOG.md` |
| Python | PyPI project metadata links Python users to the package changelog | `platforms/python/merman/CHANGELOG.md` |
| Android | The Android package README links consumers to its JNI/AAR-specific history | `platforms/android/CHANGELOG.md` |
| Apple | The Apple package README links consumers to its Swift/XCFramework-specific history | `platforms/apple/CHANGELOG.md` |
| VS Code | The unpublished extension has an independent version and release boundary | `tools/vscode-extension/CHANGELOG.md` |

For workspace-coupled packages, keep the target package entry at `Unreleased` during preparation and stamp it with the same intended tag date as the root entry immediately before immutable preflight. Each projection should contain only user-visible behavior, migrations, compatibility notes, and performance claims verified for that surface. Independently versioned surfaces keep their own version and publication date.

README files are ordinary source documentation. Review their installation examples during release preflight, but `release-version.py` never rewrites them and no post-release mode reversal is required. Confirm publication through the owning package registry or artifact workflow before recommending a released install command.

The unpublished VS Code extension, the Typst package wrapper, and `roughr-merman` own independent
version tracks. They are intentionally excluded from workspace projection. Record the workspace
runtime bundled in VSIX and Typst artifacts through provenance instead of rewriting those package
versions.

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Release Preflight

Before tagging or publishing, run:

```bash
VERSION="<version>"
SOURCE_SHA="$(git rev-parse HEAD)"
gh workflow run release-preflight.yml -f version="$VERSION" -f source_ref="$SOURCE_SHA"
```

The preflight workflow verifies release versions, package file lists, registry-independent Rust
crate publish dry-runs, Python wheels, Android AAR builds, Apple XCFramework builds, the web npm
package dry-run, platform VSIX packaging, and Flutter
`dart pub publish --dry-run`. It does not publish to any registry.

Record `VERSION` and `SOURCE_SHA` with the preflight run. The run must be green for that exact immutable commit, not merely for a branch name that can move while preflight is running. Create the tag only through the `Tag And Push` step after that run succeeds.

For local spot checks, run the normal Rust and platform gates:

```bash
cargo nextest run --cargo-quiet
python3 scripts/artifact_profile_recipe.py cli-release --build-host --locked
python3 -m py_compile \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/tool/android-smoke.py
bash -n scripts/build-apple-xcframework.sh platforms/ios/build-ios.sh platforms/flutter/build-ios.sh platforms/flutter/build-desktop.sh
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

On macOS with Xcode:

```bash
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

For Flutter:

```bash
cd platforms/flutter
flutter pub get
flutter analyze
dart format --set-exit-if-changed lib example
dart pub publish --dry-run
```

The Flutter dry run should be executed from a clean working tree. The release workflow injects
generated Android, iOS, macOS, Windows, and Linux native artifacts and then publishes with
`--force`; a full local pub package dry run should first run the same artifact injection steps from
`.github/workflows/release-flutter.yml`.

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

Normal Web releases must use `release-web.yml`. It stages every missing exact package version,
checks every tarball integrity, and only then moves the requested `alpha`, `beta`, `rc`, or `latest`
tag as a recoverable group operation. Do not publish a member manually: a bare `npm publish` can
leave the package group on divergent versions or tags.

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
cargo run --locked -p xtask -- profile-budget check-deps --profile typst-wasm --artifact-profile typst-wasm
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
reuse. Do not package the private raw Cargo output under `target/wasm-build/`. The final package must also contain
`merman_package.manifest.json`; it binds the verified artifact to the frozen wrapper/license source
snapshot, and the packaging transaction must fail before replacing the prior version if live source
or any staged byte changes.

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

After the primary release exists, run platform publish workflows manually:

```bash
RELEASE_TAG="v<version>"
gh workflow run release-python.yml -f release_tag="$RELEASE_TAG" -f publish_to_pypi=true
gh workflow run release-android.yml -f release_tag="$RELEASE_TAG"
gh workflow run release-apple.yml -f release_tag="$RELEASE_TAG"
gh workflow run release-web.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish_to_npm=true
gh workflow run vscode-extension.yml -f source_ref="$RELEASE_TAG"
gh workflow run homebrew.yml
```

The VS Code workflow currently packages and verifies platform VSIX artifacts only; Marketplace or
Open VSX publishing requires a separate credential-backed release workflow.

Do not rely on a manual `release-flutter.yml` run for pub.dev publication. A manual run still builds,
injects native artifacts, analyzes, formats, and performs `dart pub publish --dry-run`, but the real
`dart pub publish --force` step only runs from the pushed `v*` tag.

For a workflow-only recovery after a release tag already exists, Python, Android, and Apple may use
the updated workflow definition from `main`, but they still check out and verify the immutable
release-tag commit and tree. For workflows that continue to expose `source_ref`, use
`source_ref=main` only when source code and manifest versions are unchanged and the new commits only
fix CI or release workflow behavior.

## Follow-On Registry Work

- Add Android Maven Central publishing after Central Portal credentials and signing secrets are configured.
- Add device-level Flutter smoke coverage after a stable CI target is chosen for each platform.

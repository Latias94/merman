# Releasing

Status: draft release operator guide.
Last updated: 2026-07-09

Merman releases use a preflight-first flow. Run the release preflight workflow against the intended
source ref and version before any registry or GitHub Release publication. After preflight passes,
push a `v*` tag whose version matches every package manifest that will publish in that release.

## Release Workflows

| Workflow | Publishes | Channel |
| --- | --- | --- |
| `release-preflight.yml` | Nothing; dry-run/build verification only | GitHub Actions artifacts |
| `release.yml` | `merman-cli` binary archives and installers | GitHub Release |
| `release-crates.yml` | Rust workspace crates | crates.io |
| `release-apple.yml` | `Merman.xcframework-<tag>.zip` and checksum | GitHub Release artifact upload |
| `release-python.yml` | `merman` wheels for Linux, macOS, and Windows | GitHub Release + PyPI |
| `release-flutter.yml` | `merman` with injected Android, iOS, macOS, Windows, and Linux native artifacts | pub.dev |
| `release-android.yml` | `merman-android-<tag>.aar` | GitHub Release |
| `release-web.yml` | `@mermanjs/web` TypeScript/WASM package | npm |
| `vscode-extension.yml` | Platform-specific `merman-vscode` VSIX artifacts | GitHub Actions artifacts |
| `homebrew.yml` | Nothing; Homebrew/core formula health check only | Homebrew |

Most platform publish workflows are manual `workflow_dispatch` workflows that accept `release_tag`
and `source_ref` inputs. This lets a fixed workflow on `main` build assets for an existing release
tag without moving the tag. Flutter is the exception: pub.dev automated publishing only accepts
GitHub Actions runs triggered by a pushed git tag, so `release-flutter.yml` publishes from the `v*`
tag push and uses manual runs for validation only. The crates.io workflow is idempotent for
already-published crate versions, so a rerun can continue after a partial publish caused by registry
propagation delays. For unpublished crates, it performs `cargo publish --dry-run --locked`
immediately before the real publish, after upstream workspace dependencies in the same release have
become visible in crates.io.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io | `CARGO_REGISTRY_TOKEN` repository secret |
| pub.dev | Trusted Publishing / OIDC configured for `merman`, this repository, `release-flutter.yml`, and the release tag pattern |
| PyPI | Trusted Publishing / OIDC configured for `merman` and `release-python.yml` |
| npm | Trusted Publishing / OIDC configured for `@mermanjs/web`, this repository, `release-web.yml`, and the `npm` environment after the package exists |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |
| VS Code Marketplace | Not configured. Marketplace publishing would need `VSCE_PAT`, an explicit publish job, and VSIX provenance verification before enabling. |

Publish jobs use GitHub Environments (`crates.io`, `pypi`, `pub.dev`, `npm`, and `github-release`).
Configure required reviewers on those environments if publication should require explicit approval.

Android Maven Central publishing is credential-blocked. Android now declares Maven publication
metadata, but Central Portal credentials, signing secrets, and a dedicated publish job still need to
be configured.

VS Code Marketplace publishing is credential-blocked. `.github/workflows/vscode-extension.yml`
packages and verifies platform VSIX artifacts only; Marketplace publication needs a dedicated
publish job, `VSCE_PAT`, and artifact provenance verification before it is enabled.

The PyPI project `merman` exists. Keep PyPI Trusted Publishing configured for owner `Latias94`,
repository `merman`, workflow `release-python.yml`, and environment `pypi`. A PyPI Pending
Publisher is only needed before the first trusted publish of a new project name.

The npm package `@mermanjs/web` exists. Configure npm Trusted Publishing for workflow file
`release-web.yml` and GitHub environment `npm`. Subsequent trusted publishes automatically include
npm provenance; the workflow does not need `--provenance`. A manual first publish is only needed if
the package name changes and the new npm package does not exist yet.

The npm publish job is intentionally small: it runs on GitHub-hosted Ubuntu with Node 24, enters the
`npm` environment, requests `id-token: write`, downloads the package artifact, and runs plain
`npm publish` with the validated dist-tag. Do not add `NPM_TOKEN`, `NODE_AUTH_TOKEN`,
`--provenance`, `provenance=false`, `NPM_CONFIG_PROVENANCE=false`, checkout, build, or test steps to
that job. npm Trusted Publishing supplies the OIDC identity and provenance for public packages.

The Apple workflow currently publishes a zipped `Merman.xcframework` and checksum as GitHub Release
assets. It does not yet make the repository directly consumable as a remote SwiftPM package with a
`.binaryTarget(url:checksum:)`, because that checksum must be known and committed before the release
tag. Treat direct remote SwiftPM support as a separate release-manifest design task.

Homebrew installs `merman-cli` from the formula in `homebrew/core`; it is not published directly by
this repository. After a stable release, Homebrew's autobump flow should pick up the new GitHub tag.
Use `homebrew.yml` or `brew livecheck merman-cli` to verify formula freshness and run a smoke test
against the installed Homebrew package. Pre-release tags are intentionally ignored by that workflow
because Homebrew/core tracks stable versions.

## Release Surface Status

Before tagging, check the declared release surface contract:

```bash
VERSION="<version>"
python3 scripts/verify-release-surfaces.py
python3 scripts/release-status.py --version "$VERSION" --view maintainer
python3 scripts/release-status.py --version "$VERSION" --view public
```

After publication, add `--probe --format json` when network access and registry tools are available.
The JSON output keeps declared release state separate from observed registry status, so a
credential-blocked or artifact-only channel is not confused with a missing publish.

## Version Checklist

Before tagging, verify these versions match the intended release:

- `Cargo.toml` `[workspace.package].version`
- `platforms/flutter/pubspec.yaml` `version`
- `platforms/web/package.json` `version`
- `tools/vscode-extension/package.json` `version`; VS Marketplace requires stable SemVer in the
  extension manifest, so use `0.8.0` for workspace release `0.8.0-alpha.3`. The VSIX package step
  reads the workspace release version and adds the pre-release marker when needed.
- `platforms/android/build.gradle.kts` `version`
- `platforms/python/merman/pyproject.toml` `project.version`; pre-releases should use the PEP 440
  spelling, for example `0.8.0a3` for workspace release `0.8.0-alpha.3`, while final releases use
  the SemVer spelling, for example `0.7.0`

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Release Preflight

Before tagging or publishing, run:

```bash
VERSION="<version>"
gh workflow run release-preflight.yml -f version="$VERSION" -f source_ref=main
```

The preflight workflow verifies release versions, package file lists, registry-independent Rust
crate publish dry-runs, Python wheels, Android AAR builds, Apple XCFramework builds, the web npm
package dry-run, platform VSIX packaging, and Flutter
`dart pub publish --dry-run`. It does not publish to any registry.

For local spot checks, run the normal Rust and platform gates:

```bash
cargo nextest run --cargo-quiet
cargo build --release --locked -p merman-cli
python3 -m py_compile \
  scripts/release-status.py \
  scripts/verify-release-surfaces.py \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/tool/android-smoke.py
python3 scripts/verify-release-surfaces.py
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
npm run prepack
npm pack --dry-run
```

Normal web releases should use `release-web.yml` instead of local `npm publish` once npm Trusted
Publishing is configured for `@mermanjs/web`. If npm Trusted Publishing is unavailable and a
maintainer must publish locally, derive the same dist-tag as the workflow and pass it explicitly:

```bash
RELEASE_TAG="v<version>"
VERSION="${RELEASE_TAG#v}"
case "$VERSION" in
  *-alpha.*) NPM_DIST_TAG="alpha" ;;
  *-beta.*) NPM_DIST_TAG="beta" ;;
  *-rc.*) NPM_DIST_TAG="rc" ;;
  *-*) echo "unsupported prerelease tag: $RELEASE_TAG" >&2; exit 1 ;;
  *) NPM_DIST_TAG="latest" ;;
esac
npm publish --access public --tag "$NPM_DIST_TAG"
```

Prerelease packages must never be published with a bare `npm publish`.

For local VS Code VSIX validation:

```bash
cargo build --release --locked -p merman-lsp -p merman-cli
cd tools/vscode-extension
npm ci
npm test
npm run prepare:binaries
target="$(node -p 'process.platform + "-" + process.arch')"
npm run package -- --target "$target" --out "merman-vscode-${target}.vsix"
npm run verify:vsix -- --vsix "merman-vscode-${target}.vsix" --platform "$target" --target "$target"
```

Set `MERMAN_RELEASE_VERSION` when packaging a VSIX from a checkout whose workspace version does not
match the intended release. The verifier checks the stable VS Code manifest version and the
pre-release marker against that release version.

Before changing browser WASM presets or Typst package artifacts, also run the surface-specific
gates:

```bash
cargo run -p xtask -- wasm-size-matrix --surface browser
cargo run -p xtask -- wasm-size-matrix --surface typst
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

The web package build uses `wasm-pack --profile wasm-size`, so CI and local release machines need
`wasm-pack` 0.15.0 or newer. `npm run prepack --prefix platforms/web` also checks the generated
default `browser-full` wasm against `docs/release/WASM_SIZE_BUDGETS.json`.

`@mermanjs/web` publishes the `browser-full` artifact under the default import path and exposes
public subpaths for `@mermanjs/web/core`, `@mermanjs/web/render`,
`@mermanjs/web/render-only`, `@mermanjs/web/ascii`, and `@mermanjs/web/full`.
`merman-typst-plugin` is the Typst-compatible transport and must remain separate from
browser/wasm-bindgen artifacts.

## Tag And Push

```bash
VERSION="<version>"
git tag "v${VERSION}"
git push origin "v${VERSION}"
```

Do not move or force-update release tags after publication. Release tags are the immutable source
anchor for crates, CLI artifacts, and platform assets.

`release.yml` creates the primary GitHub Release and uploads CLI artifacts. Platform workflows
upload additional assets to that existing release when it is present; otherwise they leave GitHub
Actions artifacts for manual attachment.

After the primary release exists, run platform publish workflows manually:

```bash
RELEASE_TAG="v<version>"
gh workflow run release-python.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish_to_pypi=true
gh workflow run release-android.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG"
gh workflow run release-apple.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG"
gh workflow run release-web.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish_to_npm=true
gh workflow run vscode-extension.yml -f source_ref="$RELEASE_TAG"
gh workflow run homebrew.yml
```

The VS Code workflow currently packages and verifies platform VSIX artifacts only; Marketplace or
Open VSX publishing requires a separate credential-backed release workflow.

Do not rely on a manual `release-flutter.yml` run for pub.dev publication. A manual run still builds,
injects native artifacts, analyzes, formats, and performs `dart pub publish --dry-run`, but the real
`dart pub publish --force` step only runs from the pushed `v*` tag.

For a workflow-only recovery after a release tag already exists, use `source_ref=main` only when the
source code and manifest versions are unchanged and the new commits only fix CI/release workflow
behavior.

## Follow-On Registry Work

- Add Android Maven Central publishing after Central Portal credentials and signing secrets are configured.
- Add device-level Flutter smoke coverage after a stable CI target is chosen for each platform.

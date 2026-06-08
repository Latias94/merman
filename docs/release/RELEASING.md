# Releasing

Status: draft release operator guide.
Last updated: 2026-06-06

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

Most platform publish workflows are manual `workflow_dispatch` workflows that accept `release_tag`
and `source_ref` inputs. This lets a fixed workflow on `main` build assets for an existing release
tag without moving the tag. Flutter is the exception: pub.dev automated publishing only accepts
GitHub Actions runs triggered by a pushed git tag, so `release-flutter.yml` publishes from the `v*`
tag push and uses manual runs for validation only. The crates.io workflow is idempotent for
already-published crate versions, so a rerun can continue after a partial publish caused by registry
propagation delays.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io | `CARGO_REGISTRY_TOKEN` repository secret |
| pub.dev | Trusted Publishing / OIDC configured for `merman`, this repository, `release-flutter.yml`, and the release tag pattern |
| PyPI | Trusted Publishing / OIDC configured for `merman` and `release-python.yml` |
| npm | Trusted Publishing / OIDC configured for `@mermanjs/web`, this repository, `release-web.yml`, and the `npm` environment after the package exists |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |

Publish jobs use GitHub Environments (`crates.io`, `pypi`, `pub.dev`, `npm`, and `github-release`).
Configure required reviewers on those environments if publication should require explicit approval.

Android Maven Central publishing is intentionally not enabled yet. Android now declares Maven
publication metadata, but Central Portal credentials and signing secrets still need to be configured.

The PyPI project `merman` exists. Keep PyPI Trusted Publishing configured for owner `Latias94`,
repository `merman`, workflow `release-python.yml`, and environment `pypi`. A PyPI Pending
Publisher is only needed before the first trusted publish of a new project name.

The npm package `@mermanjs/web` exists. Configure npm Trusted Publishing for workflow file
`release-web.yml` and GitHub environment `npm`. Subsequent trusted publishes automatically include
npm provenance; the workflow does not need `--provenance`. A manual first publish is only needed if
the package name changes and the new npm package does not exist yet.

The Apple workflow currently publishes a zipped `Merman.xcframework` and checksum as GitHub Release
assets. It does not yet make the repository directly consumable as a remote SwiftPM package with a
`.binaryTarget(url:checksum:)`, because that checksum must be known and committed before the release
tag. Treat direct remote SwiftPM support as a separate release-manifest design task.

## Version Checklist

Before tagging, verify these versions match the intended release:

- `Cargo.toml` `[workspace.package].version`
- `platforms/flutter/pubspec.yaml` `version`
- `platforms/web/package.json` `version`
- `platforms/android/build.gradle.kts` `version`
- `platforms/python/merman/pyproject.toml` `project.version`; pre-releases should use the PEP 440
  spelling, for example `0.7.0a2` for workspace release `0.7.0-alpha.2`

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Release Preflight

Before tagging or publishing, run:

```bash
gh workflow run release-preflight.yml -f version=0.7.0-alpha.2 -f source_ref=main
```

The preflight workflow verifies release versions, package file lists, Python wheels, Android AAR
builds, Apple XCFramework builds, the web npm package dry-run, and Flutter
`dart pub publish --dry-run`. It does not publish to any registry.

For local spot checks, run the normal Rust and platform gates:

```bash
cargo nextest run --cargo-quiet
cargo build --release --locked -p merman-cli
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

For local npm validation or emergency first package creation:

```bash
cd platforms/web
npm ci
npm run build
npm run smoke
npm pack --dry-run
npm publish --access public --tag alpha
```

Normal web releases should use `release-web.yml` instead of local `npm publish` once npm Trusted
Publishing is configured for `@mermanjs/web`.

## Tag And Push

```bash
git tag v0.7.0-alpha.2
git push origin v0.7.0-alpha.2
```

Do not move or force-update release tags after publication. Release tags are the immutable source
anchor for crates, CLI artifacts, and platform assets.

`release.yml` creates the primary GitHub Release and uploads CLI artifacts. Platform workflows
upload additional assets to that existing release when it is present; otherwise they leave GitHub
Actions artifacts for manual attachment.

After the primary release exists, run platform publish workflows manually:

```bash
gh workflow run release-python.yml -f release_tag=v0.7.0-alpha.2 -f source_ref=v0.7.0-alpha.2 -f publish_to_pypi=true
gh workflow run release-android.yml -f release_tag=v0.7.0-alpha.2 -f source_ref=v0.7.0-alpha.2
gh workflow run release-apple.yml -f release_tag=v0.7.0-alpha.2 -f source_ref=v0.7.0-alpha.2
gh workflow run release-web.yml -f release_tag=v0.7.0-alpha.2 -f source_ref=v0.7.0-alpha.2 -f publish_to_npm=true
```

Do not rely on a manual `release-flutter.yml` run for pub.dev publication. A manual run still builds,
injects native artifacts, analyzes, formats, and performs `dart pub publish --dry-run`, but the real
`dart pub publish --force` step only runs from the pushed `v*` tag.

For a workflow-only recovery after a release tag already exists, use `source_ref=main` only when the
source code and manifest versions are unchanged and the new commits only fix CI/release workflow
behavior.

## Follow-On Registry Work

- Add Android Maven Central publishing after Central Portal credentials and signing secrets are configured.
- Add device-level Flutter smoke coverage after a stable CI target is chosen for each platform.

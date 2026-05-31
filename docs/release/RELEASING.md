# Releasing

Status: draft release operator guide.
Last updated: 2026-05-31

Merman releases are tag-driven. Push a `v*` tag whose version matches every package manifest that
will publish in that release.

## Release Workflows

| Workflow | Publishes | Channel |
| --- | --- | --- |
| `release.yml` | `merman-cli` binary archives and installers | GitHub Release |
| `release-crates.yml` | Rust workspace crates | crates.io |
| `release-apple.yml` | `Merman.xcframework-<tag>.zip` and release-tag `Package.swift` patch | GitHub Release + SwiftPM |
| `release-python.yml` | `merman` wheels for Linux, macOS, and Windows | GitHub Release |
| `release-flutter.yml` | `merman` with injected Android, iOS, macOS, Windows, and Linux native artifacts | pub.dev |
| `release-android.yml` | `merman-android-<tag>.aar` | GitHub Release |

All workflows can be run manually with `workflow_dispatch`, but they must be run from a `v*` tag.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io | `CARGO_REGISTRY_TOKEN` repository secret |
| pub.dev | Trusted Publishing / OIDC configured for `merman` |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |

Android Maven Central and Python PyPI publishing are intentionally not enabled yet. Android needs
Maven namespace/signing/POM metadata. Python needs a manylinux/musllinux wheel policy before PyPI.

## Version Checklist

Before tagging, verify these versions match the intended release:

- `Cargo.toml` `[workspace.package].version`
- `platforms/flutter/pubspec.yaml` `version`
- `platforms/android/build.gradle.kts` `version`
- `platforms/python/merman/pyproject.toml` `project.version`

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Local Preflight

Run the normal Rust and platform gates before tagging:

```bash
cargo nextest run --cargo-quiet
cargo build --release --locked -p merman-cli
python3 -m py_compile \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/tool/android-smoke.py
bash -n scripts/build-apple-xcframework.sh platforms/ios/build-ios.sh platforms/flutter/build-desktop.sh
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

## Tag And Push

```bash
git tag v0.7.0
git push origin v0.7.0
```

The Apple workflow patches `Package.swift` on the release tag so SwiftPM consumers can resolve the
remote binary target checksum. It force-updates only the tag, not the branch.

Multiple workflows attach assets to the same GitHub Release. The cargo-dist workflow is configured
to upload to an existing release when another workflow creates it first.

## Follow-On Registry Work

- Add Android Maven Central publishing after Maven coordinates and signing metadata are confirmed.
- Add Python PyPI publishing after wheels are produced with an accepted Linux compatibility policy.
- Add device-level Flutter smoke coverage after a stable CI target is chosen for each platform.

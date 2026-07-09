---
name: merman-release
description: Merman release operator workflow. Use when preparing a new Merman version, updating changelog or release notes, bumping package versions, running release preflight, creating or verifying a tag release, dispatching platform publish workflows, recovering failed release CI, or checking published registry and GitHub Release state.
---

# Merman Release

Run releases as a preflight-first, evidence-backed path. Keep `docs/release/RELEASING.md` as the
source of truth for commands and surface inventory; use this skill to follow the same process every
time and to avoid the release traps already found in this repo.

## Read First

From the repository root, read these before editing or publishing:

- `docs/release/RELEASING.md`
- `docs/release/PACKAGE_SURFACES.md`
- `docs/release/PUBLISH_ORDER.md`
- the top entry of `CHANGELOG.md`
- the manifests listed in the `Version Checklist` section of `docs/release/RELEASING.md`

Completion criterion: the target version, source ref, release channel, publish surfaces, and version
files are known before any tag or registry action.

## Release Notes

Write release notes for users first, then maintainers.

- Start each top-level changelog entry with `## [version] - YYYY-MM-DD`.
- Follow with a short user-facing summary paragraph.
- Add `### Highlights` with only the changes users should scan first.
- Give new crates, packages, editor integrations, and platform surfaces their own short bullets when
  they change how users install or integrate Merman.
- Mention migration or compatibility impact explicitly.
- Include PR references where they help GitHub release notes, for example `(#23)`.
- Prune no-op bullets such as "point to the right docs", broad internal cleanup, duplicate package
  metadata notes, or implementation details users cannot act on.
- Do not manually wrap prose inside a bullet. Let the formatter or editor wrap display text.
- When polishing wording, use the repo's release-note voice and run `$humanizer` if it is available.

Completion criterion: the changelog can stand alone as the GitHub Release body, has no duplicate
items, and explains new user-facing surfaces without turning into a commit log.

## Version Sync

Update every release-facing version before preflight:

- Rust workspace version in `Cargo.toml`.
- Cargo crate versions and workspace dependency pins implied by the release.
- `platforms/web/package.json`.
- `platforms/flutter/pubspec.yaml`.
- `platforms/android/build.gradle.kts`.
- `platforms/python/merman/pyproject.toml`, using PEP 440 for prereleases such as `0.8.0a3`.
- `tools/vscode-extension/package.json`; VS Code uses stable SemVer in the manifest and marks
  prerelease VSIX builds during packaging.
- Platform changelogs and package README compatibility sections when the published surface changes.
- `packages/typst/merman/README.md` version mapping when the Typst package embeds a new workspace
  renderer line.

Completion criterion: every release-facing manifest and compatibility table names the same intended
workspace release, with platform-specific spelling only where the registry requires it.

## Preflight

Run the manual release preflight before tagging:

```bash
gh workflow run release-preflight.yml -f version=<version> -f source_ref=main
```

Wait for the run to complete and inspect failed jobs before tagging. Do not treat a local build as a
substitute for preflight, because preflight covers registry-independent package dry-runs, platform
artifacts, VSIX packaging, Flutter dry-run publishing, and WASM size gates.

Completion criterion: `release-preflight.yml` is green for the exact version and source ref that
will be tagged.

## Tag And Publish

After preflight passes, tag the intended source commit:

```bash
git tag v<version>
git push origin v<version>
```

Watch the tag-triggered workflows first:

- `release.yml` creates the GitHub Release and CLI/LSP artifacts.
- `release-crates.yml` publishes crates.io packages in dependency order.
- `release-flutter.yml` publishes to pub.dev from the tag-triggered run.

After the GitHub Release exists, dispatch the platform workflows from the tag:

```bash
gh workflow run release-python.yml -f release_tag=v<version> -f source_ref=refs/tags/v<version> -f publish_to_pypi=true
gh workflow run release-android.yml -f release_tag=v<version> -f source_ref=refs/tags/v<version>
gh workflow run release-apple.yml -f release_tag=v<version> -f source_ref=refs/tags/v<version>
gh workflow run release-web.yml -f release_tag=v<version> -f source_ref=refs/tags/v<version> -f publish_to_npm=true
gh workflow run vscode-extension.yml -f source_ref=refs/tags/v<version>
```

Completion criterion: every intended release workflow has a successful latest run for the target
version, and skipped jobs are expected by channel rules rather than accidental.

## Verification

Verify the published state, not only workflow success:

- GitHub Release is not draft, has the intended prerelease/stable state, and contains expected CLI,
  LSP, source, checksum, Python wheel, Android AAR, and Apple XCFramework assets when those surfaces
  are part of the release.
- crates.io shows the published Rust crate versions.
- npm shows `@mermanjs/web@<version>` and the correct dist-tag: `alpha`, `beta`, `rc`, or `latest`.
- PyPI and pub.dev show the intended package versions after their workflows publish.
- VS Code workflow artifacts exist; Marketplace publishing is not enabled unless a separate release
  decision added it.
- `main` CI is green after any release-workflow fixes.

Completion criterion: registries and GitHub Release state agree with the workflow matrix, and the
working tree is clean or only contains explicitly reported unrelated user changes.

## Recovery

Classify a failed release before changing code:

- Source or manifest failure: fix the source on `main`, rerun preflight, and create a new tag only if
  nothing has been externally published for the broken tag.
- Workflow-only failure after a tag exists: fix the workflow on `main`, then rerun manual workflows
  with `source_ref=refs/tags/<tag>` so the artifact source remains the release tag.
- Registry partial publish: rerun only idempotent or remaining workflows. Do not republish packages
  that the registry already accepted.
- After any external registry publish, treat the release tag as immutable. Do not move it unless the
  maintainer explicitly accepts the registry and provenance risk.

Known traps from `0.8.0-alpha.3`:

- `cargo pkgid` version parsing must handle both `#version` and `@version` forms.
- `gh release view` and `gh release upload` jobs without checkout need `GH_REPO`.
- cargo-dist workflow updates must be regenerated and checked with `dist generate --check`.
- `npm pack --json` must not be polluted by lifecycle script logs; run verification before packing
  and pack release artifacts with scripts disabled when the workflow already ran `prepack`.
- Typst package compatibility can fail on missing README version mapping even when size budgets pass.

Completion criterion: the recovery path preserves published artifacts, keeps the tag source
explainable, and ends with successful replacement runs for every failed release surface.

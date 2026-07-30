---
name: merman-release
description: Merman release operator workflow. Use when preparing a new Merman version, updating changelog or release notes, bumping package versions, running release preflight, creating or verifying a tag release, dispatching platform publish workflows, recovering failed release CI, or checking published registry and GitHub Release state.
---

# Merman Release

Use this skill as the release-operator checklist. `docs/release/RELEASING.md` explains the public workflow, but package manifests, artifact profiles, platform descriptors, owner workflows, and actual registry or GitHub evidence remain the authority for a release decision. README text and a central release-status file are never machine release authority.

## Read First

Read:

- `docs/release/RELEASING.md`
- `docs/release/PACKAGE_SURFACES.md`
- `docs/release/PUBLISH_ORDER.md`
- the top entry of `CHANGELOG.md`
- the `Version Checklist` in `docs/release/RELEASING.md`

Before changing anything, identify the target version, immutable source commit, release channel, and intended publish surfaces.

## Authorization Boundary

Treat the request as `prepare` unless the maintainer explicitly authorizes `ship` for a concrete version, source commit, channel, and surface set.

- `prepare` may update source files, run local checks, dispatch non-publishing preflight when requested, and inspect external state read-only.
- `ship` may create or push a tag, dispatch a publishing workflow, modify a GitHub Release, or mutate a registry only within the named scope.
- Pushing a release tag starts one inseparable tag-triggered publication bundle: `release.yml`, `release-crates.yml`, and `release-flutter.yml`. The maintainer must authorize all three tag-triggered surfaces; authorization for only part of that bundle does not authorize a tag push.
- Python, Android, Apple, Web, VS Code, and Homebrew are separately authorized surfaces.
- Preparation, a green preflight, an existing plan, a partial prior release, or authorization from an earlier release never implies current shipping authorization.

If scope is incomplete, report what is ready and stop before the first external mutation.

## Prepare

### Release Notes

Write the changelog for users:

- Keep an in-progress entry as `## [version] - Unreleased`; immediately before immutable preflight, replace `Unreleased` with the intended tag date in `YYYY-MM-DD` form.
- State installation, integration, migration, compatibility, or behavior changes users can act on.
- Remove duplicate metadata and internal implementation detail.
- Keep each Markdown paragraph or bullet on one physical line.
- Use the repository's release-note voice; use `$writing-great-skills` for an evidence-backed range report and `$humanizer` when prose needs a final polish.

### Version Projection And Documentation Review

Treat `Cargo.toml` `[workspace.package].version` as the workspace release authority. Use the exact projection and validation commands in the `Version Checklist` of `docs/release/RELEASING.md`.

Do not hand-edit generated version projections. If the command is interrupted, preserve the partial diff and rerun the same command; the workspace authority is written last.

README files are ordinary documentation. Review their install examples, package names, feature names, and supported-platform statements before preflight, but do not switch them between source and registry modes or make them part of a version projection. A cancelled or completed release therefore has no README state to restore.

VS Code, Typst, and `roughr-merman` have independent version axes. Update them only when that surface is being released, and preserve their bundled Merman provenance.

### Local Contract And Preflight

Run the owner-owned contract checks listed in `docs/release/RELEASING.md`, including exact artifact recipes, package build/load smoke tests, ABI checks, release legal material, and the requested preflight workflows.

Resolve the intended commit to a 40-character `SOURCE_SHA`. Use the exact preflight dispatch from `docs/release/RELEASING.md`, passing that immutable SHA instead of a branch. Wait for every job and diagnose failures before tagging. A local build is not a substitute for preflight.

Preparation is complete when version projections and release notes are committed, the release-ready check passes, and preflight is green for the exact version and `SOURCE_SHA`.

Apple source compatibility is a compiler-floor contract. The Swift 5.9/Xcode 15.2 CI job must pass for the exact source commit; a newer Swift compiler does not prove that floor. GitHub's `macos-14` hosted image starts retirement brownouts on 2026-10-05 and retires on 2026-11-02, so move this exact check to a maintained runner or toolchain before the brownouts. If no equivalent runner is available, block Apple shipping instead of weakening or silently skipping the check.

## Ship

Do not continue here without explicit `ship` authorization for the current release.

Use only the `Tag And Push` commands in `docs/release/RELEASING.md`. They must verify that `HEAD` is the preflighted `SOURCE_SHA`, create the tag at that explicit commit, and push that tag without moving it.

Watch the tag-triggered cargo-dist, crates.io, and pub.dev workflows first. After the GitHub Release exists, use only the exact platform dispatch commands in `docs/release/RELEASING.md`; do not reconstruct them from memory or from this skill.

Skipped jobs must be explained by channel rules. A manual surface is not authorized merely because the tag-triggered bundle succeeded.

## Verify

Use direct GitHub, registry, and artifact evidence documented in `docs/release/RELEASING.md`. Confirm that:

- the GitHub Release state and assets match the channel and configured target matrix;
- crates.io, npm, PyPI, and pub.dev show only the surfaces that were actually published;
- separately authorized Android, Apple, Web, VS Code, and Homebrew outputs match their declared channel semantics;
- `main` remains green after any release-workflow repair.

Workflow success is not publication evidence. Registry and GitHub state must agree with the release contract.

## Recovery

Classify the failure before changing anything:

- Source or manifest failure before publication: fix source, rerun preflight, and use a new tag only if nothing external accepted the broken version.
- Workflow-only failure after tagging: fix the workflow on `main`, then rerun the authorized workflow against `refs/tags/<tag>` so provenance remains anchored to the release.
- Partial registry publication: rerun only idempotent or unfinished surfaces; never republish a version a registry accepted.
- Any accepted external publication makes the release tag immutable unless the maintainer explicitly accepts the provenance risk of changing it.

Diagnosis and local fixes are `prepare` work. Rerunning a publisher, uploading an asset, changing a tag, or mutating a registry requires fresh `ship` authorization.

## Known Traps

- `cargo pkgid` may print versions with either `#` or `@`.
- GitHub Release jobs without checkout need `GH_REPO`.
- cargo-dist workflow changes require `dist generate --check`.
- `npm pack --json` output must not be contaminated by lifecycle logs.
- Typst compatibility includes README version mapping, not only size gates.

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

## Operating Mode And Authorization Boundary

Treat every release request as `prepare` unless the maintainer's current instruction explicitly
authorizes `ship` for a concrete version, source ref, release channel, and set of publish surfaces.

- `prepare` may edit version files and release notes, run local checks, dispatch the non-publishing
  preflight when requested, and inspect release or registry state read-only. It must stop after
  reporting the prepared commit and preflight evidence.
- `ship` may create or push a tag, dispatch a publishing workflow, create or modify a GitHub
  Release, or mutate a registry only within the explicitly authorized scope.
- A pushed release tag starts one inseparable tag-triggered publication bundle: `release.yml`
  creates cargo-dist GitHub Release artifacts, `release-crates.yml` publishes crates.io, and
  `release-flutter.yml` publishes pub.dev. Authorization for only one of those surfaces does not
  authorize creating or pushing the tag; the maintainer must authorize all three tag-triggered surfaces.
- A request to prepare, bump, validate, run preflight, recover, or continue does not authorize
  external release mutations. A green preflight, an existing release plan, or authorization from a
  previous release also does not authorize them.
- If any part of the ship scope is missing or ambiguous, state the exact external actions that are
  ready, ask the maintainer for explicit authorization, and stop before the first mutation.

Completion criterion: the active mode and, for `ship`, the authorized version, source ref, channel,
and surfaces are recorded in the conversation before any external release mutation.

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

Treat `Cargo.toml` `[workspace.package].version` as the only workspace release authority. Project
and verify its registry-specific forms with:

```bash
python3 scripts/release-version.py set --version <version>
python3 scripts/release-version.py
```

The command validates the complete projection in memory, then updates Cargo dependency
requirements, workspace and fuzz lock metadata, Web package and lock metadata, Python's PEP 440
form, Android and Flutter manifests, CocoaPods metadata, iOS framework versions, and generated
README installation examples. Run it in an exclusive Git worktree. If it is interrupted, keep the
partial diff and rerun the same command; the workspace authority is written last so the update is
recoverable without a private journal. A changed version always returns the README to
source-candidate mode. Do not edit those projections independently.

Immediately before preflight, switch the generated README blocks to exact registry commands and
run the release-ready gate:

```bash
python3 scripts/release-version.py set-readme-mode \
  --mode registry --version <version>
python3 scripts/release-version.py check --version <version>
```

Registry mode prepares exact commands inside release artifacts; it does not claim that crates.io,
npm, or any other independently published channel is already live. Commit every path printed by
the command before tagging, including the root manifest, root README, and all projected package
READMEs. If release preparation is cancelled, run the same command with `--mode source`. Do not
publish, tag, or dispatch a publishing workflow while the release-ready check fails.

After a successful publication, keep the released commit and tag in `registry` mode. Do not switch
the same version back to `source`, because that would describe a published release as unpublished.
When development advances to the next version, `set --version <next-version>` returns the generated
commands to `source` mode automatically. If `main` remains on the released version, leaving it in
`registry` mode is the truthful state.

The VS Code extension, Typst package wrapper, and `roughr-merman` have independent version axes.
Do not derive them from the workspace release. Update their versions only for a release of that
specific package, and record the bundled workspace renderer through artifact provenance or the
Typst compatibility mapping. Update platform changelogs and package README compatibility sections
when the published surface changes.

Completion criterion: every workspace-coupled manifest names the root release, the README is in
exact registry mode for that release, and each independent package version and bundled
workspace-runtime provenance are internally consistent.

## Preflight

Validate the repository-owned release contract before dispatching anything:

```bash
python3 scripts/verify-release-surfaces.py
python3 scripts/test_release_workflow_security.py
python3 scripts/release-status.py --version <version> --view maintainer
```

Then use the exact `release-preflight.yml` dispatch command from `docs/release/RELEASING.md`. That
document and the verified workflow files own release inputs and commands; do not copy a parallel
command matrix into this skill. Wait for the run to complete and inspect failed jobs before tagging.
Do not treat a local build as a substitute for preflight, because preflight covers
registry-independent package dry-runs, platform artifacts, VSIX packaging, Flutter dry-run
publishing, and WASM size gates.

Completion criterion: `release-preflight.yml` is green for the exact version and source ref that
will be tagged.

## Tag And Publish

This section is a mandatory authorization gate, not an automatic continuation from preflight.
Before running any command below, confirm the active mode is `ship` and that the maintainer has
explicitly authorized the exact version, source ref, release channel, and publish surfaces in the
current request. Otherwise report the green preflight and stop in `prepare` mode.

A tag push is not surface-selective. Treat `release.yml`, `release-crates.yml`, and
`release-flutter.yml` as the tag-triggered publication bundle and confirm all three are explicitly
authorized before creating or pushing the release tag. Python, Android, Apple, Web, VS Code, and
Homebrew remain separately authorized manual surfaces. If any tag-triggered publisher is not ready
or is intentionally excluded, remain in `prepare`; do not push a tag and hope that workflow-level
conditions will approximate the missing authorization.

After preflight passes, tag the intended source commit:

```bash
git tag v<version>
git push origin v<version>
```

Watch the tag-triggered workflows first:

- `release.yml` is cargo-dist output for both user-facing executables declared in
  `dist-workspace.toml`: CLI and LSP archives, installers, and checksums. Regenerate and inspect the
  cargo-dist manifest when that package list or target matrix changes.
- `release-crates.yml` publishes crates.io packages in dependency order.
- `release-flutter.yml` publishes to pub.dev from the tag-triggered run.

After the GitHub Release exists, use the exact platform dispatch commands from
`docs/release/RELEASING.md`. The Python, Android, Apple, Web, VS Code, and Homebrew workflows have
different publication semantics and must not be inferred from a stale copy here. In particular,
`vscode-extension.yml` currently produces verified GitHub Actions VSIX artifacts; it does not
publish to Marketplace. The standalone LSP assets attached to the GitHub Release come from
cargo-dist, independently of VSIX packaging.

Completion criterion: every intended release workflow has a successful latest run for the target
version, and skipped jobs are expected by channel rules rather than accidental.

## Verification

Verify the published state, not only workflow success:

- GitHub Release is not draft, has the intended prerelease/stable state, and contains the expected
  cargo-dist CLI and LSP archives, installers, and checksums for every configured target. It has
  Python wheels, Android AAR, and Apple XCFramework assets only when their separate upload workflows
  are part of the release.
- crates.io shows the published Rust crate versions.
- npm shows `@mermanjs/web@<version>` and the correct dist-tag: `alpha`, `beta`, `rc`, or `latest`.
- PyPI and pub.dev show the intended package versions after their workflows publish.
- VS Code workflow artifacts exist; Marketplace publishing is not enabled unless a separate release
  decision added it.
- `main` CI is green after any release-workflow fixes.

Use `scripts/release-status.py` with the maintainer and public views documented in
`docs/release/RELEASING.md`; do not replace its contract with hand-written `gh`/registry assumptions
inside this skill.

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

Recovery diagnosis and local workflow fixes remain `prepare` work. Rerunning a publishing workflow,
uploading an asset, changing a tag, or mutating a registry requires a fresh `ship` authorization for
the named version, source ref, channel, and affected surfaces; an incident or partial publish does
not waive the authorization gate.

Known traps from `0.8.0-alpha.3`:

- `cargo pkgid` version parsing must handle both `#version` and `@version` forms.
- `gh release view` and `gh release upload` jobs without checkout need `GH_REPO`.
- cargo-dist workflow updates must be regenerated and checked with `dist generate --check`.
- `npm pack --json` must not be polluted by lifecycle script logs; run verification before packing
  and pack release artifacts with scripts disabled when the workflow already ran `prepack`.
- Typst package compatibility can fail on missing README version mapping even when size budgets pass.

Completion criterion: the recovery path preserves published artifacts, keeps the tag source
explainable, and ends with successful replacement runs for every failed release surface.

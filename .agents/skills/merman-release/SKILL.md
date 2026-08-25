---
name: merman-release
description: Merman release operator workflow. Use when preparing a new Merman or independently versioned support-crate release, updating changelog or release notes, bumping package versions, running release preflight, creating or verifying a tag release, dispatching platform publish workflows, recovering failed release CI, or checking published registry and GitHub Release state.
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

Before changing anything, classify the work as development, a workspace release, or an explicitly authorized channel-only prerelease test. Identify the immutable source commit, package version when applicable, release channel, and intended publish surfaces. Do not invent the next workspace version merely because one channel needs packaging work.

## Authorization Boundary

Treat the request as `prepare` unless the maintainer explicitly authorizes `ship` for a concrete version, source commit, channel, and surface set.

- `prepare` may update source files, run local checks, dispatch non-publishing preflight when requested, and inspect external state read-only.
- `ship` may create or push a tag, dispatch a publishing or deployment workflow, modify a GitHub Release, or mutate a registry only within the named scope.
- Pushing a release tag starts one inseparable tag-triggered publication bundle: `release.yml`, `release-crates.yml`, and `release-flutter.yml`. The maintainer must authorize all three tag-triggered surfaces; authorization for only part of that bundle does not authorize a tag push.
- Python, Android, Apple, Web, VS Code, Homebrew, and GitHub Pages are separately authorized surfaces. Pages authorization must name a reviewed ref and approved 40-character commit; Web/npm or CI authorization does not cover dispatching `pages-deploy.yml` or a matching-path push to `main` or `master`.
- Preparation, a green preflight, an existing plan, a partial prior release, or authorization from an earlier release never implies current shipping authorization.

If scope is incomplete, report what is ready and stop before the first external mutation.

## Channel-Only Prerelease Tests

Use this path only when the maintainer explicitly authorizes one channel to test publication under an existing prerelease version without claiming a new workspace release.

- Keep the package manifest, embedded runtime catalog, and package-group version exact and consistent.
- Resolve `source_ref` to a reviewed 40-character commit and bind that SHA into the verified package-group manifest. Require a registry provenance attestation when the workflow can produce one; record a manual first-publication bootstrap separately and never describe an unattested registry artifact as provenance-backed.
- Treat a same-named workspace tag as a different publication snapshot when its commit differs. State that boundary in current release documentation and avoid cross-channel byte-identity claims.
- Keep unrelated tag-triggered crates, binaries, and language-binding publishers out of scope. A channel-only test never authorizes moving a tag or republishing another registry.
- Keep external registry mutation behind the ordinary `ship` authorization boundary even when non-publishing preflight and tarball generation are green.

## Prepare

### Release Notes

Write the changelog for users:

- While the next workspace version is unselected, keep the root entry as `## [Unreleased]`. Once the maintainer selects a workspace version, rename it to `## [version] - Unreleased`; immediately before immutable preflight, replace `Unreleased` with the intended tag date in `YYYY-MM-DD` form.
- Treat the root `CHANGELOG.md` as the canonical project-wide release narrative. For every intended publish surface that owns a package-local changelog, update that file as a curated projection of the same release delta: include only changes, migrations, compatibility notes, and verified performance claims that affect that package's users.
- Keep workspace-coupled package changelog entries at `Unreleased` during preparation, then stamp them with the same intended tag date as the root entry immediately before immutable preflight. Independently versioned surfaces keep their own version and publication date.
- Review `platforms/node/CHANGELOG.md`, `platforms/flutter/CHANGELOG.md`, `platforms/python/merman/CHANGELOG.md`, `platforms/android/CHANGELOG.md`, and `platforms/apple/CHANGELOG.md` when their surfaces are in scope. Review `tools/vscode-extension/CHANGELOG.md` only for the extension's independent release. Do not create per-crate changelogs or copy the complete root entry into every package.
- For an authorized channel-only publication, leave the root workspace entry at `Unreleased`, but stamp that channel's package-local version entry with the intended publication date before the immutable package artifact is built. Confirm the date against registry evidence after publication.
- State installation, integration, migration, compatibility, or behavior changes users can act on.
- Remove duplicate metadata and internal implementation detail.
- Keep each Markdown paragraph or bullet on one physical line.
- Use the repository's release-note voice; use `$writing-great-skills` for an evidence-backed range report and `$humanizer` when prose needs a final polish.

### Version Projection And Documentation Review

Treat `Cargo.toml` `[workspace.package].version` as the workspace release authority. Use the exact projection and validation commands in the `Version Checklist` of `docs/release/RELEASING.md`.

Run the version projection only after the maintainer selects the next workspace version. A channel-only npm alpha test may reuse the current workspace prerelease version from a newer reviewed commit without selecting the next workspace release; keep the root changelog at `Unreleased` and leave workspace versions unchanged.

Run `release-version.py set` only from a clean linked release worktree, never the primary checkout.
Use the exact npm version pinned by `playground/package.json`; the command rejects tool drift. Cargo,
npm, and platform owners prepare their manifests and locks in disposable state, after which the
coordinator validates the full projection and source preimage, checks one binary patch, and applies
that patch once. Preparation and pre-apply failures leave the caller unchanged. Do not hand-edit
generated projections or copy partial files from the disposable worktree; fix the owner failure and
rerun the same command.

README files are ordinary documentation and are not generated version projections. Before immutable preflight, perform a release-state documentation pass across the root README, every package README shipped by an authorized surface, and the closest installation guides:

- Make the latest published registry or GitHub Release version explicit when the source tree is ahead of it.
- Use the exact target prerelease version in Cargo dependency examples that will ship inside the target crate or archive; do not leave a moving default-branch Git dependency in release-facing examples.
- Keep source-only Git commands only when the surrounding text labels them as source installs and the command pins a reviewed 40-character commit.
- Remove stale future tense such as “after this version is published”, old published baselines, and candidate wording once external publication evidence exists.
- Review package-local READMEs included by `cargo package`, cargo-dist, npm, Python, Flutter, Apple, Android, Typst, or VSIX packaging, not only the repository root README.

Use a broad README version search before preflight and again after every publication or recovery:

```bash
rg -n --glob '**/README.md' --glob '**/CHANGELOG.md' \
  '0\.[0-9]+\.[0-9]+-(alpha|beta|rc)\.[0-9]+|\[[^]]+\] - Unreleased|published registry packages can still|candidate from Git|[Aa]fter .*is published|[Ww]hen .*is published|git\s*=\s*"https://github\.com/Latias94/merman' \
  README.md CHANGELOG.md crates distribution docs platforms tools
```

Classify every match instead of blindly replacing it. Historical reports may retain historical wording; current installation guidance must match the intended release state. A cancelled or completed release has no generated README mode to restore, but a successful or partially recovered publication can require a new documentation commit so `main` stops describing an already-published version as unavailable.

VS Code, Typst, and `roughr-merman` have independent version axes. Update them only when that surface is being released, and preserve their bundled Merman provenance.

### Independent Implementation Crates

Treat an independently versioned implementation crate such as `roughr-merman` as a compatibility boundary, not merely another workspace member.

- Keep the workspace path dependency's declared version aligned with the selected independent-crate release. Use Cargo's ordinary compatible requirement (`0.12.3` admits later `0.12.x` patches); reserve an exact requirement for time-bounded incident containment with an explicit removal condition.
- Apply Cargo SemVer to `0.y.z` crates: a patch release preserves source compatibility, while a public breaking change increments `y` (`0.12.x` to `0.13.0`). A removed or renamed method, changed signature, removed default feature, or upgraded public dependency whose types cross the crate API cannot ship in a patch unless the candidate provides a compatibility adapter.
- Before publishing a patch, compare its public API with the latest published version in the same compatibility line. Treat a compatibility-restoration patch after an accidental breaking release as incident recovery, not precedent for carrying future breaking changes in patch versions.
- Run `cargo semver-checks check-release -p roughr-merman --color always` before publishing. The registry-selected `0.12.3` baseline is the compatibility floor for later `0.12.x` patches; do not reopen the already-published `0.12.1` to `0.12.3` recovery history as a release blocker. Pin the CI tool version through the owning workflow rather than relying on a moving local installation.
- Identify every published Merman stable or prerelease whose Cargo requirement can admit the candidate. Compile each one from crates.io in a fresh temporary project without copying a lockfile, first against the candidate package and then, after publication, against ordinary registry resolution. A workspace build is not evidence because its path dependency bypasses the registry version choice that consumers make.
- Require both the previous stable lane and the current prerelease lane to compile before publication completes. Keep this matrix narrow: one newest published dependent per distinct dependency/API contract is sufficient.
- Publish the compatible replacement before yanking a broken predecessor. Wait for registry visibility, confirm fresh resolution selects the replacement in every dependent lane, then yank the predecessor; existing lockfiles may continue using a yanked crate.

An independent patch is ready only when the API comparison is non-breaking, the focused crate tests pass, every admitted published dependent compiles from a fresh resolution, and the workspace dependency floor and lock projections name the candidate version.

### Local Contract And Preflight

Run the owner-owned contract checks listed in `docs/release/RELEASING.md`, including exact artifact recipes, package build/load smoke tests, ABI checks, release legal material, and the requested preflight workflows.

Keep release smoke tests strict about user-observable behavior and loose about valid representation choices. Binary-format checks may require signatures, bounded structure, and a real payload, but must accept legal token or whitespace forms. Async protocol checks must require the expected responses, bounds, and exit behavior while allowing valid notification ordering. When a final-archive smoke fails, reproduce against that exact archive before changing product code; do not refactor the product merely to satisfy an incidental serializer or scheduler ordering.

Resolve the intended commit to a 40-character `SOURCE_SHA`. Use the exact preflight dispatch from `docs/release/RELEASING.md`, passing that immutable SHA instead of a branch. Wait for every job and diagnose failures before tagging. A local build is not a substitute for preflight.

Preparation is complete when version projections and release notes are committed, the release-ready check passes, preflight is green for the exact version and `SOURCE_SHA`, and every tag-triggered package fits the current registry upload constraints. Treat an exit-zero package dry-run that reports a server-enforced size or content hint as unresolved release evidence.

For crates.io, inspect the derived topological batches before shipping. The credentialed workflow
must emit a prepared receipt for every batch, and its result receipt must reach `complete` with each
registry checksum equal to the locally prepared `.crate` digest before the next batch begins. The
receipt schema is owned by `distribution/crates-io/receipt-schema-v1.json`; do not generalize it to
other registries.

Within one crates.io batch, every missing member must complete `cargo publish --dry-run --locked`
before any member is uploaded. A later dry-run failure must therefore leave the whole batch
unpublished.

For Flutter, keep the package archive and its source/tree/version/digest receipt together. The
pub.dev job must verify and safely extract that archive with the trusted workflow revision before
configuring Dart OIDC credentials; never replace this boundary with direct `tar -x` extraction.

### npm Package Groups

For Web or Node publication, treat the verified package-group manifest order as publication order. Publish dependency or platform packages first and the default Web package or Node loader last.

npm Trusted Publisher authorizes `npm publish`, not a later `npm dist-tag` repair. The owner script must preflight every existing exact version, integrity, and requested tag before mutating the registry, publish only missing versions directly under the final tag, verify each postcondition, and skip matching members on retry. Stop before publication when an existing integrity or tag conflicts; recover that tag with an explicitly authorized maintainer credential. Do not reintroduce a synthetic staging tag or OIDC-incompatible promotion step.

When npm package-group code or its workflow changes, run the focused owner checks before dispatching:

```bash
python3 -m unittest scripts.test_node_package_group scripts.test_web_package_group scripts.test_release_workflow_security
node --test platforms/node/tests/build-candidate.test.mjs
```

Apple source compatibility is a compiler-floor contract. The Swift 5.9/Xcode 15.2 CI job must pass for the exact source commit; a newer Swift compiler does not prove that floor. GitHub's `macos-14` hosted image starts retirement brownouts on 2026-10-05 and retires on 2026-11-02, so move this exact check to a maintained runner or toolchain before the brownouts. If no equivalent runner is available, block Apple shipping instead of weakening or silently skipping the check.

## Ship

Do not continue here without explicit `ship` authorization for the current release.

Use only the `Tag And Push` commands in `docs/release/RELEASING.md`. They must verify that `HEAD` is the preflighted `SOURCE_SHA`, create the tag at that explicit commit, and push that tag without moving it.

Watch the tag-triggered cargo-dist, crates.io, and pub.dev workflows first. After the GitHub Release exists, use only the exact platform dispatch commands in `docs/release/RELEASING.md`; do not reconstruct them from memory or from this skill.

Skipped jobs must be explained by channel rules. A manual surface is not authorized merely because the tag-triggered bundle succeeded.

### GitHub Pages Deployment

Under explicit Pages `ship` authorization, dispatch `.github/workflows/pages-deploy.yml`, bind the
run to the approved commit, wait for both build and deploy, and inspect failed logs before retrying:

```bash
PAGES_REF="<reviewed-branch-or-tag>"
PAGES_SHA="<approved-40-character-commit>"

gh workflow run pages-deploy.yml --ref "$PAGES_REF"
PAGES_RUN_ID="$(gh run list \
  --workflow pages-deploy.yml \
  --event workflow_dispatch \
  --commit "$PAGES_SHA" \
  --limit 1 \
  --json databaseId \
  --jq '.[0].databaseId')"
test -n "$PAGES_RUN_ID"
test "$(gh run view "$PAGES_RUN_ID" --json headSha --jq .headSha)" = "$PAGES_SHA"
if ! gh run watch "$PAGES_RUN_ID" --exit-status; then
  gh run view "$PAGES_RUN_ID" --log-failed
  exit 1
fi

PAGES_DEPLOYMENT_ID="$(gh api --method GET repos/{owner}/{repo}/deployments \
  -f environment=github-pages \
  -f sha="$PAGES_SHA" \
  -f per_page=10 \
  --jq 'map(select(.task == "deploy")) | first | .id')"
test -n "$PAGES_DEPLOYMENT_ID"
PAGES_URL="$(gh api "repos/{owner}/{repo}/deployments/$PAGES_DEPLOYMENT_ID/statuses" \
  --jq 'map(select(.state == "success" and ((.environment_url // "") != ""))) | first | .environment_url')"
test -n "$PAGES_URL"
curl --fail --location --silent --show-error --output /dev/null "$PAGES_URL"
printf '%s\n' "$PAGES_URL"
```

The workflow run, successful `github-pages` deployment status, and reachable deployment URL must
all refer to the approved commit before reporting Pages complete.

## Verify

Use direct GitHub, registry, and artifact evidence documented in `docs/release/RELEASING.md`. Confirm that:

- the GitHub Release state and assets match the channel and configured target matrix;
- crates.io, npm, PyPI, and pub.dev show only the surfaces that were actually published;
- every crates.io batch result receipt is `complete`, with no `pending_recovery` or `mismatch`
  state and no dependent batch started before its predecessors matched;
- separately authorized Android, Apple, Web, VS Code, and Homebrew outputs match their declared channel semantics;
- `main` remains green after any release-workflow repair.

Workflow success is not publication evidence. Registry and GitHub state must agree with the release contract.

### Crates.io Receipt Inspection

Download the exact `release-crates.yml` receipt artifact for read-only inspection. Use the
preflighted source SHA, not the workflow head SHA of a manual recovery run:

```bash
CRATES_RUN_ID="<release-crates-run-id>"
SOURCE_SHA="<preflighted-40-character-source-sha>"
CRATES_ATTEMPT="$(gh run view "$CRATES_RUN_ID" --json attempt --jq .attempt)"
RECEIPTS_DIR="target/crates-io-receipts/$CRATES_RUN_ID"
RECEIPT_SCHEMA="distribution/crates-io/receipt-schema-v1.json"

gh run download "$CRATES_RUN_ID" \
  --name "crates-io-receipts-${SOURCE_SHA}-attempt-${CRATES_ATTEMPT}" \
  --dir "$RECEIPTS_DIR"

jq -s -e --slurpfile schema "$RECEIPT_SCHEMA" '
  def fields_match($shape):
    (($shape.required - keys) | length == 0)
    and ((keys - ($shape.properties | keys)) | length == 0);
  ($schema[0]) as $schema
  | all(.[];
      fields_match($schema)
      and (.source | fields_match($schema.properties.source))
      and (.toolchain | fields_match($schema.properties.toolchain))
      and all(.packages[];
        fields_match($schema["$defs"].package)
        and (.artifact | fields_match($schema["$defs"].package.properties.artifact))
        and (.registry | fields_match($schema["$defs"].package.properties.registry))))
' "$RECEIPTS_DIR"/batch-*.json

jq -s -e --arg source_sha "$SOURCE_SHA" '
  all(.[];
    .schema_version == 1
    and .schema == "distribution/crates-io/receipt-schema-v1.json"
    and .channel == "crates.io"
    and .kind == "topological-batch"
    and .source.commit == $source_sha)
  and ((map(select(.state == "prepared") | .batch_index) | sort)
    == (map(select(.state != "prepared") | .batch_index) | sort))
  and all(.[];
    .state == "prepared"
    or (.state == "complete"
      and all(.packages[];
        .registry.observed_checksum != null
        and .registry.observed_checksum == .artifact.sha256)))
' "$RECEIPTS_DIR"/batch-*.json
```

These commands only download and inspect evidence; they do not authorize recovery or publication.

### Post-Publication Documentation Reconciliation

Treat each registry and artifact channel independently. After any successful publication or
recovery, query the owning registry or GitHub Release, update current installation commands and
availability prose only for the surfaces that actually published, and move
`docs/release/PUBLISH_ORDER.md` from candidate state to the published workspace baseline when the
Rust release lands. Rerun the broad README version search above and account for every match.

For npm groups, query every member's exact version integrity and complete dist-tag map, compare the
integrity with the verified package-group manifest, and verify the requested tag before declaring
the group released. First publications can expose registry-assigned tags beyond the requested tag;
record the observed state instead of assuming the requested tag is the only one. Confirm that the
published tarball already contains the dated package-local changelog, then update current README
availability prose. If the immutable tarball shipped stale documentation, record that defect and
fix the source for the next version instead of claiming the registry artifact was corrected.
Also query npm provenance attestations for every member. Do not treat an embedded
`artifacts/provenance.json` file as an npm attestation; if a manual bootstrap has none, state that
boundary in current release documentation and direct users to the verified package-group artifact
and release record instead.

Do not report the release complete while a current README installation command names an older
workspace prerelease or current prose says the published target is unavailable. Commit the
documentation reconciliation before completion, or report it as explicit unfinished release work.

## Recovery

Classify the failure before changing anything:

- Source or manifest failure before publication: fix source, rerun preflight, and use a new tag only if nothing external accepted the broken version.
- Workflow-only failure after tagging: fix the workflow on `main`, then rerun the authorized workflow against `refs/tags/<tag>` so provenance remains anchored to the release.
- Partial crates.io publication: retain the receipt artifact and rerun from the exact same immutable
  source. A rerun must download the prior attempt receipts; a new workflow run supplies the prior
  run id through `recovery_run_id`. Deterministic re-packaging must match the prior source, tree,
  toolchain, plan, manifest, artifact identities, and every existing registry checksum; matching
  members are skipped and missing members continue only after the current batch barrier completes.
- A crates.io `pending_recovery` result means visibility or response status is unresolved. Wait and
  rerun the same workflow; do not issue a second manual publish attempt while acceptance is unknown.
- A crates.io `mismatch` result is an incident requiring an explicit maintainer decision. Never
  auto-yank, silently resume, or publish dependent batches.
- Partial publication on other registries: rerun only idempotent or unfinished surfaces; never
  republish a version a registry accepted.
- Any accepted external publication makes the release tag immutable unless the maintainer explicitly accepts the provenance risk of changing it.

Diagnosis and local fixes are `prepare` work. Rerunning a publisher, uploading an asset, changing a tag, or mutating a registry requires fresh `ship` authorization.

## Known Traps

- `cargo pkgid` may print versions with either `#` or `@`.
- `cargo publish` re-packages from source; crates.io integrity is therefore proven by deterministic
  local `.crate` preparation plus the registry checksum, not by claiming Cargo uploaded an arbitrary
  prebuilt file.
- GitHub Release jobs without checkout need `GH_REPO`.
- cargo-dist workflow changes require `dist generate --check`.
- `npm pack --json` output must not be contaminated by lifecycle logs.
- npm Trusted Publisher cannot run the package group's old post-publish `dist-tag` promotion; publish directly under the final tag and make retries integrity-aware.
- Workflow contract tests should follow stable step IDs and provenance data flow; display labels and local variable names are not release invariants.
- Recovery admission should require a non-empty subset of allowed paths plus semantic and trusted-source checks, not require every allowed path to change.
- `dart pub publish --dry-run` can succeed while reporting a package too large for server upload. Inspect its final compressed size before tagging; split the package surface instead of silently dropping documented targets.
- Typst compatibility includes README version mapping, not only size gates.

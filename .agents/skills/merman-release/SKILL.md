---
name: merman-release
description: Prepare, publish, verify, or recover Merman workspace and independently versioned releases. Use for version bumps, release notes, immutable preflight, registry credentials, publishing workflows, and external package-manager or Typst submissions.
---

# Merman Release

Operate the selected release through its owning manifests, artifact descriptors, workflows, and
registries. Use [Releasing](../../../docs/release/RELEASING.md) for commands and
[Publish Order](../../../docs/release/PUBLISH_ORDER.md) for dated evidence. Read the relevant
section when its step or channel applies; status documents describe observations rather than
granting permission or defining executable contracts.

## 1. Establish scope and authorization

Inspect the working tree and existing conversation authorization. Record the selected version,
delivery surfaces, intended date, and source identity; distinguish a new workspace release,
independent package release, channel backfill, publication recovery, or status observation. For a
status-only request, proceed directly to step 5 without version projection or publishing preflight.
For a readiness review, distinguish product defects, normal candidate preparation, external
channel setup and intentionally deferred channels. Review and fix preparation issues without
treating the review as authorization to create tags or publish.

Prepare source, documentation, checks, and a reviewable result within the request. Continue
shipping and necessary recovery when the maintainer has already authorized that release and its
surfaces. Resolve and record the exact source during preparation; a request to publish a selected
release need not already contain its commit hash. Ask only when scope expands, a checksum or
provenance decision needs the maintainer, or an upstream rule explicitly requires personal review.
Complete independent preparation before presenting that decision.

Use [Package Surfaces](../../../docs/release/PACKAGE_SURFACES.md) when delivery scope is unclear.
Workspace tags trigger Cargo and CLI/LSP owners; other registries, deployment, independently
versioned packages, and external repositories have their own owners. Include only requested
surfaces. Before the first external write, verify its target, immutable source, and authorization.

Review the complete [Release Workflows](../../../docs/release/RELEASING.md#release-workflows)
table before publication, including channels outside the static FFI/native contract. In the existing
dated Publish Order entry, record each relevant workspace-coupled channel as `selected` or
`deferred`. For every deferred channel, record its current independently verified published version
and evidence, or explicitly state that availability could not be verified. Honor a requested subset;
preflight coverage and shared workspace versions do not expand its publication scope.

Done: the requested result and each owner's authorized operations are explicit; unrelated work
remains intact.

## 2. Prepare versions and user documentation

For a new workspace version, follow the
[Version Checklist](../../../docs/release/RELEASING.md#version-checklist).
Use its clean linked-worktree projection rather than hand-editing generated versions or locks.
For independent packages, preserve their own version, bundled runtime mapping, ABI, and date.

Follow the Version Checklist's generated-output refresh after projection, including committed
Rustdoc receipts and the current native-memory contracts' changed lockfile identity. Keep historical
measurements and performance limits unchanged; version projection alone does not refresh these owners.

Version each public contract against its latest published baseline. Keep new unpublished theme
contracts at version 1 and consolidate one unreleased compatibility change once. Preserve
published ABI boundaries, historical evidence identities and unknown-version rejection; see
[Contract Version Baseline](../../../docs/release/RELEASING.md#contract-version-baseline).

- For prerelease graph admission or a channel-only prerelease, read
  [prereleases](references/prereleases.md).
- For an independent implementation-crate release, read
  [independent crates](references/independent-crates.md).
- Use `repo-changelog` for a verified full delta from the previous published release, linked
  PRs/issues, contributor credit, user outcomes, and migration notes. Project that narrative onto
  the package-local changelogs of in-scope SDKs.
- Keep unselected development changes at `[Unreleased]`; date a selected release before freezing
  its artifact. Independently published wrappers use their own intended publication date.
- Prepare normal install/import examples as documentation of the target package. Keep public binary
  download instructions for deferred channels at their verified published versions. Keep detailed
  availability evidence, pending PRs, source identities, and receipts in operator records; summarize
  actual channel delivery in the public Release body.
- For a selected headless SDK release, describe the `standard`, `math`, and `full` choices and
  supported transports in user-facing notes; claim downloads are available only after the SDK
  publisher succeeds and the matching GitHub Release assets are verified.
- Describe ordinary prebuilt SVG products through the current artifact recipes and
  [host guide](../../../docs/release/PREBUILT_PRODUCTS.md). Keep source feature choices,
  task-specific Web packages, output codecs, font services and resource policies distinct.
  State theme maturity and recorded family/output scope without inferring qualification from
  a preset's availability or a successful best-effort render.
- Describe performance in representative user workflows with bounded evidence. For optional font
  measurement, explain its approximate cost and when improved layout warrants enabling it.
- Use plain fenced code blocks for Mermaid source examples in changelogs so GitHub displays the
  source. Inspect the actual README, changelog, legal files, and generated assets inside each
  candidate package.

Done: version projections pass, user documentation matches the target APIs, and packaged bytes
contain the reviewed documentation.

## 3. Freeze and prove the candidate

Run [contribute-merman-pr](../contribute-merman-pr/SKILL.md) checks for changed surfaces, then the
release-owned checks and exact dispatch in
[Release Preflight](../../../docs/release/RELEASING.md#release-preflight).
Resolve the candidate to a 40-character source commit and record its tree.

Require successful immutable preflight for the selected workspace version and source before a
workspace tag. Independent owners use their documented source, compatibility, package and artifact
checks. Inspect upload-size warnings even when a dry run exits zero. Preserve documented target
coverage and measure the final compressed upload, including native/WASM artifacts.
Use `scope=release` for this gate. `scope=bindings` qualifies unreleased host products, skips
release-readiness owners and never issues reusable release-preflight evidence. A prior binding
candidate or PR CI run does not qualify a later version-projected release commit.

For the SDK owner, run qualification only from a clean source tree. Preflight qualification uploads
Actions artifacts but does not publish them; retain its run ID only when that run's `head_sha` equals
the selected release commit.

Read [Required Credentials](../../../docs/release/RELEASING.md#required-credentials) for the
selected registries. Distinguish configured bindings, live token exchange, and successful upload.
Existing bindings generally span versions; a new package name may need bootstrap and registration.
Inspect the descriptor's complete npm group, including newly added platform packages, for
first-publication bootstrap and Trusted Publisher setup before starting the group publication.
Follow the frozen-group bootstrap procedure in Required Credentials: hold an actual publishing
run before its upload job, or manually reconcile the complete build-only group. A partially
bootstrapped candidate is not permission to rebuild or bypass observation-only recovery.
Use the workflows' pinned tools and compiler-floor lanes rather than copying versions into this
skill. Diagnose first failures against the exact produced artifact.

Done: source/tree/version and final artifacts are frozen, every required owner check passes, and
credentials and upload constraints are ready for the authorized publication.

## 4. Publish through the owners

Follow [Tag And Push](../../../docs/release/RELEASING.md#tag-and-push) and each selected workflow's
actual inputs. Keep workflow revision, artifact source, producing run/attempt, and receipts
distinct. Workspace, Flutter and independent grammar tags retain their owning trigger contracts.

Dispatch every selected manual owner using Tag And Push; tag-triggered owners keep their
owning trigger. Preflight and Actions artifacts do not establish public delivery.

Apple, Android and Python default to nonpublishing candidates. Their `publish=true` path
requires the matching immutable tag and an existing GitHub Release; PyPI additionally requires
`publish_to_pypi=true`. Android release uploads also require the verified Maven group and
signed Central bundle. Preparing that bundle does not submit or promote it in Central.

For Apple remote SwiftPM rollout, use the complete thin delivery tree with matching sources
and the frozen binary URL/checksum. Native producer and delivery-repository commits are
separate identities. Creating or tagging that repository is a separately scoped operation;
uploading the package archives alone does not establish public URL-based installation.

For Flutter, follow the pushed `flutter-v<version>` tag owner: freeze target assets and the
thin facade, upload native assets to its existing non-draft GitHub Release, verify every
manifest-pinned public download, then publish the same frozen facade to pub.dev. Manual runs
validate only. Keep the public native downloads available before exposing the dependent facade.

If the SDK surface is selected, publish its full matrix after the primary GitHub Release exists,
using the manual SDK publisher and successful source-matched qualification run. Do not treat
preflight or Actions artifacts as public downloads. Follow the publisher's exact source/tag checks,
verify all attached assets, and update the release body with download choices only after
publication succeeds.

For Typst, Homebrew, Scoop, WinGet, Conda-forge, Nixpkgs, or Pages, read
[external channels](references/external-channels.md) before submission or deployment.
Observe accepted versions and existing bot/maintainer PRs before writing externally.

Done: each authorized upload or submission has an identified owner result. A submitted upstream
PR is reported as pending acceptance until its registry serves the version.

## 5. Reconcile delivery

Query each selected owner's registry or actual public GitHub Release assets independently.
Compare its exact versions, checksums, tags, provenance, assets and package contents with the frozen
owner's evidence; use the existing verifiers and receipt schemas. Cargo requires both API and
sparse-index checksums. npm groups require every member's integrity and requested dist-tag. Verify registry provenance for OIDC
uploads; explicitly record the missing attestation of a manual first-publication bootstrap and
preserve its accepted version.

Record observed results and outstanding upstream review in Publish Order. Mark checklist items
complete only for the state actually proven: prepared, submitted, merged, or available. Inspect
channel semantics: AAR/XCFramework release assets do not establish Maven Central/SwiftPM delivery.
For Apple, follow the public-download verification in Tag And Push. Every selected channel
requires its publishing owner to succeed and matching public delivery evidence before completion.
Record Apple asset delivery separately from public SwiftPM repository resolution, and Android
Maven repository/bundle delivery separately from Central availability. Flutter completion requires
both the exact native asset downloads and the reconciled pub.dev facade.
For SDK publication, reconcile all 18 archives and their 18 SHA-256 sidecars against the verified
producer run and the matching GitHub Release before marking the SDK available; the release body
must link only to assets confirmed present on that release.
The public Release body must list the channels actually delivered and the deferred channels still
on their verified older versions. Keep selected channels awaiting delivery explicitly pending;
retain detailed evidence in Publish Order without treating deferred channels as unfinished scope.
Confirm the latest commit's relevant CI after workflow repairs; identify cancellations or external
failures separately from source regressions.

Done: delivered surfaces have registry evidence, pending surfaces have a concrete next condition,
and documentation reflects those observations.

## 6. Recover without changing accepted artifacts

For a failed or partial publisher, read
[recovery](references/recovery.md) before retrying. Reuse the original frozen artifact and producing
attempt evidence; only proven-unattempted members may upload. Preserve uncertain acceptance
across retries and later batches. Observe accepted matching versions, stop on mismatches, and retain
immutable release tags.

For Apple asset recovery, follow
[Tag And Push](../../../docs/release/RELEASING.md#tag-and-push): rerun only the failed upload job
in the original run, retaining its build artifact and the owner's existing-asset comparison.

Continue idempotent recovery covered by the existing shipping request. A new version, new surface,
tag move, conflicting registry bytes, or incident yank needs the corresponding explicit decision.

Done: delivery is reconciled, or the remaining upstream/external condition and exact retained
evidence are reported without claiming completion.

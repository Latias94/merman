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

Done: the requested result and each owner's authorized operations are explicit; unrelated work
remains intact.

## 2. Prepare versions and user documentation

For a new workspace version, follow the
[Version Checklist](../../../docs/release/RELEASING.md#version-checklist).
Use its clean linked-worktree projection rather than hand-editing generated versions or locks.
For independent packages, preserve their own version, bundled runtime mapping, ABI, and date.

- For prerelease graph admission or a channel-only prerelease, read
  [prereleases](references/prereleases.md).
- For an independent implementation-crate release, read
  [independent crates](references/independent-crates.md).
- Use `repo-changelog` for a verified full delta from the previous published release, linked
  PRs/issues, contributor credit, user outcomes, and migration notes. Project that narrative onto
  the package-local changelogs of in-scope SDKs.
- Keep unselected development changes at `[Unreleased]`; date a selected release before freezing
  its artifact. Independently published wrappers use their own intended publication date.
- Prepare normal install/import examples as documentation of the target package. Keep availability,
  pending PRs, source identities, and receipts in operator records.
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

Read [Required Credentials](../../../docs/release/RELEASING.md#required-credentials) for the
selected registries. Distinguish configured bindings, live token exchange, and successful upload.
Existing bindings generally span versions; a new package name may need bootstrap and registration.
Use the workflows' pinned tools and compiler-floor lanes rather than copying versions into this
skill. Diagnose first failures against the exact produced artifact.

Done: source/tree/version and final artifacts are frozen, every required owner check passes, and
credentials and upload constraints are ready for the authorized publication.

## 4. Publish through the owners

Follow [Tag And Push](../../../docs/release/RELEASING.md#tag-and-push) and each selected workflow's
actual inputs. Keep workflow revision, artifact source, producing run/attempt, and receipts
distinct. Workspace, Flutter and independent grammar tags retain their owning trigger contracts.

For Typst, Homebrew, Scoop, WinGet, Conda-forge, Nixpkgs, or Pages, read
[external channels](references/external-channels.md) before submission or deployment.
Observe accepted versions and existing bot/maintainer PRs before writing externally.

Done: each authorized upload or submission has an identified owner result. A submitted upstream
PR is reported as pending acceptance until its registry serves the version.

## 5. Reconcile delivery

Query each owning registry or GitHub Release independently. Compare its exact versions, checksums,
tags, provenance, assets and package contents with the frozen owner's evidence; use the existing
verifiers and receipt schemas. Cargo requires both API and sparse-index checksums. npm groups
require every member's integrity and requested dist-tag. Verify registry provenance for OIDC
uploads; explicitly record the missing attestation of a manual first-publication bootstrap and
preserve its accepted version.

Record observed results and outstanding upstream review in Publish Order. Mark checklist items
complete only for the state actually proven: prepared, submitted, merged, or available. Inspect
channel semantics: AAR/XCFramework release assets do not establish Maven Central/SwiftPM delivery.
Confirm the latest commit's relevant CI after workflow repairs; identify cancellations or external
failures separately from source regressions.

Done: delivered surfaces have registry evidence, pending surfaces have a concrete next condition,
and documentation reflects those observations.

## 6. Recover without changing accepted artifacts

For a failed or partial publisher, read
[recovery](references/recovery.md) before retrying. Reuse the original frozen artifact and producing
attempt evidence; only proven-unattempted members may upload. Preserve uncertain acceptance across
retries and later batches. Observe accepted matching versions, stop on mismatches, and retain
immutable release tags.

Continue idempotent recovery covered by the existing shipping request. A new version, new surface,
tag move, conflicting registry bytes, or incident yank needs the corresponding explicit decision.

Done: delivery is reconciled, or the remaining upstream/external condition and exact retained
evidence are reported without claiming completion.

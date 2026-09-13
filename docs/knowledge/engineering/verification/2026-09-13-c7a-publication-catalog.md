---
type: Verification Evidence
title: Publish the archive-bound native preset catalog through the verified release bundle
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,c7a,release,catalog,verification
---

# Finding and change

At `84853d1f2`, the final native Linux CLI job already generated and replayed an archive-bound
public preset catalog, but only uploaded it as a separate Actions artifact. The final gate,
attestation and GitHub Release host downloaded the earlier `verified-release-assets` bundle,
which contained no public catalog. Qualification evidence stopped before product distribution.

The native producer and Rust qualification remain unchanged. `archive_catalog` now owns the
existing public projection shared by the archive verifier and bundle assembly. After native
replay succeeds, the release gate downloads that exact source/target artifact and calls
`release_artifact_bundle.py finalize`. It copies the verified base bundle into a fresh directory,
checks that record and companion agree, and binds source, Linux target, release version and
archive SHA-256 to the base bundle. Only the public companion is attached, under
`merman-cli-x86_64-unknown-linux-gnu.preset-catalog.json`. Its size and digest join the existing
release manifest; archive bytes, adjacent archive checksums and installers stay unchanged.

The gate uploads a separately named `publication-release-assets` artifact. Registry candidate,
attestation and host jobs all download it and verify with `--require-preset-catalog` before
consuming it. The latter two jobs check out only the pinned source to run the verifier;
permissions are unchanged. The private full execution record never enters the public bundle.

This assembly code is not a qualification issuer or an arbitrary JSON importer. It trusts the
successful native verification job in the same workflow run as producer. The producer's real
Rust execution, exact CLI output comparison and fresh replay remain prerequisites. Bundle
assembly only checks projection and artifact identity; it does not reimplement semantic/visual
qualification or build a source-analysis framework. Shared Rust/SDK discovery remains unqualified,
and the CLI does not trust ambient files placed beside its binary.

# Validation

- Initial owner script run passed 109/110; the remaining failure was the explicit Python public
  helper list, which needed the new bounded `finalize_bundle` operation.
- Owner scripts then passed 113/113 and full scripts passed 592/592.
- Independent review reproduced a final-publication gap: removing the catalog and its manifest
  row still satisfied the generic pre-native verifier. The new final mode explicitly requires
  the catalog. API and actual CLI subprocess negative tests now reject that exact mutation.
- Final full script suite passed **593/593**. It includes the real Python finalize subprocess,
  unchanged archive/installers, private-record exclusion, missing/stale/changed companion,
  wrong source/target/version/archive/executable identity, tampering and rehashed wrong binding,
  plan-time injection rejection, and workflow consumer/source/dependency guards.
- `actionlint .github/workflows/release.yml` and `git diff --check` passed. Independent final
  review found no remaining issue in this scope.
- Clean-checkout replay is pending.

Commands and raw logs:

```text
python3 -m unittest discover -s scripts -p 'test_*.py'
actionlint .github/workflows/release.yml
git diff --check
```

Logs: `/tmp/theme-c7a-bundle-first.log`, `/tmp/theme-c7a-bundle-tests.log`,
`/tmp/theme-c7a-all-scripts.log`, `/tmp/theme-c7a-all-scripts-final.log`, and
`/tmp/theme-c7a-actionlint-final.log`. The first attempted run also exposed a transient Python
import indentation error during editing; it was fixed before the owner suite executed.

# Limits and next gate

This slice changes Python/workflow/documentation only. Tests use bounded archive fixtures and
mock native observations for assembly; they do not claim a new Linux native qualification or an
actual published Release. No workflow was dispatched, tag or version changed, or package published.
The producer's prior native witness remains bound to its original source record. A future
immutable Linux candidate must execute the changed workflow and validate the resulting final
asset set before this path can count as a completed artifact-profile delivery observation.
C7a remains open for that matrix, shared discovery rollout and contract freeze; C7b remains open
for long-tail bridge and provider/probe retirement. C5 remains closed.

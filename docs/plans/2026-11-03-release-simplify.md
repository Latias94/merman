---
title: Simplify release delivery and recovery
date: 2026-11-03
status: in-progress
---

# Release delivery and recovery

## Problem

The alpha publication exposed unnecessary coupling between a Flutter facade and its native
producer, an extra GitHub Release used only as an asset bucket, one-time bootstrap code left
active after its purpose was complete, and duplicated npm report persistence and recovery glue.
These increase operator work without improving the proof that consumers can install a release.

## Scope and decisions

- Work on the main development line, starting from PR #274's verified delivery changes. If that
  PR is still open, keep this change separately reviewable as a dependent PR. Do not merge it
  automatically or change the 0.9 maintenance branch.
- Future Flutter assets belong to the matching `v<version>` Merman Release. The pub.dev package
  remains `merman`. Keep the `flutter-v<version>` authentication tag and existing Trusted Publisher
  pattern so maintenance publishing remains compatible, but create no GitHub Release for that tag.
  A tag push publishes after the primary Release exists; manual tag-ref publication is opt-in and
  requires its event enabled in pub.dev. Do not create or move release tags during this refactor.
- Keep every historical Flutter Release and asset URL available. Accepted pub.dev packages and
  their manifests are immutable; this refactor affects future candidates only.
- Give Flutter native production and facade packaging separate responsibilities. A facade may
  consume an explicitly selected original native-asset artifact without rebuilding or rewriting
  its manifest. The facade receipt identifies facade source; the manifest identifies native
  source. The archive digest already binds the included manifest, so no parallel fingerprint or
  redundant identity format is needed. Version and full consumer-contract checks remain required.
- Use one read-only Flutter builder from preflight and the publishing owner. Publishing jobs
  receive verified frozen bytes, obtain credentials only at the upload boundary, and never
  execute downloaded hooks with publication credentials.
- Retire the alpha-only three-crate bootstrap workflow and dedicated tests after the three names
  have been accepted and their Trusted Publishers configured. Preserve historical receipts.
- Let the shared npm reconciler own durable report writes. Share report selection between Node
  and Web; an older report is usable only when every intervening upload was explicitly skipped.
  Missing, malformed or ambiguous history remains observation-only. Never turn the historical
  Windows incident decision into a general resend override.

## Implementation sequence

1. Repair PR #274's Apple CI staging collision separately: package and qualification output must
   use the runner's temporary directory rather than restored Cargo target cache contents.
2. Separate Flutter facade/native archive identity and prove unchanged-manifest composition.
3. Share npm atomic persistence and bounded report selection, preserving exact report provenance.
4. Replace Flutter-specific Release choreography with workspace Release asset delivery and
   one reusable candidate builder; add explicit native-artifact reuse for facade repairs.
5. Remove the completed one-time bootstrap owner, update owner selection and release guidance,
   and remove stale current-policy copies without rewriting historical observations.
6. Review the integrated changes, run owner checks and a real frozen-package consumer, commit
   focused modules, and open a reviewable PR. Inspect final-head CI and fix causal failures.

## Validation

- Flutter: archive, Native Assets and pub.dev member-reconciliation tests; reject changed native
  bytes, wrong versions, unsafe archives and unavailable public assets. Compose a modified facade
  with the original alpha manifest/assets in an isolated temporary directory and run its real
  Dart runtime and frozen-release theme consumers without publishing anything.
- npm: shared reconciler and Node/Web artifact tests; subprocess interruption and durable report
  checks; report selection with paginated GitHub responses, skipped intervening attempts,
  missing/duplicate artifacts and unknown upload outcomes. Keep source/manifest/report binding.
- Workflows: focused security/selection contracts, actionlint and high-severity zizmor at the
  repository's pinned versions. Full platform owners remain responsible for hosted toolchains.
- All changes: patch whitespace and indexed path-length checks. No Cargo product change is
  planned, so do not repeat renderer or performance qualification locally.

## Deferred work

Apple/Android reusable builders and a cross-channel dispatch summary remain the next batch.
They are useful, but are not necessary to remove the concrete recovery and Flutter release
friction covered here. Keep registry-specific owners and completion criteria; do not build a
general release state machine, custom compiler analysis, or another status database.

## Acceptance

Future Flutter publication needs no separate Flutter GitHub Release. Its authentication tag keeps
the existing publisher pattern compatible with maintenance. A facade
repair can reuse the exact qualified native assets through documented inputs. One shared npm
implementation persists reports, and both npm owners use the same safe selection behavior.
The alpha-only bootstrap is absent from active workflows. Historical downloads remain intact,
the optional manual-event configuration is explicit, and the changes are reviewable
without performing another publication.

## Implementation evidence

- The preceding PR's Apple cache collision is repaired independently in `608c997b5`: packaging
  and consumer outputs now use `RUNNER_TEMP`, preserving the packager's no-overwrite behavior.
- Flutter's thin archive composes distinct immutable facade/native sources without changing either
  schema. The native materializer restores one verified host library; public URLs and original
  manifest bytes stay unchanged.
- Preflight and release call one read-only Flutter builder. The release owner attaches target
  assets to the workspace Release, retaining the authentication tag and existing pub.dev pattern.
- npm report writes have one owner, and report recovery selects the real surviving attempt only
  across skipped uploads. The completed three-crate bootstrap workflow and tests are removed.
- Independent correctness and security reviews found one cross-run Flutter resend hole, now
  closed by checking all runs/attempts for the immutable authentication tag before a send.
- Native reuse also requires a successful producer build, the workspace tag's exact commit/tree,
  and unchanged main-Release URLs. A failed registry upload may retain a successful native build;
  a different source or unqualified native producer cannot be imported.
- Local validation passes 319 focused tests plus 115 release-regression tests, followed by 111
  targeted history/security checks after the cross-run repair. Actionlint 1.7.12 and high-severity
  zizmor 1.30.1 report no findings for the changed workflows.
- The complete hygiene Python suite passes 1,052 tests before the final native-producer checks;
  the follow-up workflow/history/surface subset passes 120 tests.
- A real Dart 3.12.2/macOS ARM64 consumer uses the corrected frozen facade with the original alpha
  native manifest/assets. Official pub.dev dry-run has zero warnings; runtime/formula/callback and
  frozen-release theme consumers pass. This proves native Dart composition, not a newly executed
  Flutter app or runtime coverage for every target. Hosted candidate qualification remains needed.

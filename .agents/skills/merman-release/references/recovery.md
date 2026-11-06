# Publication recovery

Read [Registry Propagation And Recovery](../../../../docs/release/RELEASING.md#registry-propagation-and-recovery)
and the affected workflow before retrying. Use its verifier and registry-specific receipt schema;
keep receipt interpretation in the owner implementation rather than a second ad hoc parser.

## Identify what failed

- Before any external acceptance, a source/manifest fix needs a newly frozen candidate and owner
  evidence. Inspect every potentially accepting owner before deciding that a tagged version is
  unused.
- After a workflow-only failure, repair the workflow and operate against the original immutable
  artifact source. Distinguish workflow head SHA from source SHA.
- After an uncertain response or partial publication, retain the original run/attempt, artifact,
  source/tree/version and recovery report. Registry absence alone does not prove no upload occurred.
- On conflicting registry bytes, stop dependent publication and prepare the maintainer's explicit
  checksum/provenance decision. An incident yank has a separate owner and credential.

## Use the registry's recovery owner

For Cargo, inspect every topological batch's prepared and result receipt from the exact producing
attempt. Every missing member must pass its locked publish dry-run before the batch's first upload.
Cargo re-packages from source; the retained source/tool/manifest identity and deterministic
`.crate` digest must match both registry API and sparse index. Matching accepted members are
skipped. Missing or incompatible history preserves observation-only mode across retries and later
batches. `pending_recovery` leaves acceptance unresolved; `complete` requires all checksums.

For Web/Node npm groups, retain the original package-group tarballs and verified manifest order.
Use the workflow's `recovery_run_id` and exact source SHA for a new recovery run; a failed-job retry
stays in its original run. Independently rebuilt tarballs can differ despite the same source.
Reconcile every exact version, integrity and dist-tag before writes. Publish proven-unattempted
members, or explicit package-PUT permission rejections qualified by Registry Propagation And
Recovery, directly under the final tag. The current package-group operator does not repair
dist-tags; resolve a tag conflict through a separately authorized operation after checking the
current registry permissions. Verify registry
provenance rather than treating an embedded provenance file as an attestation.
For first-publication bootstrap, follow Required Credentials' same-artifact procedure. A
nonpublishing candidate has no upload history report; cross-run observation-only recovery
must not be used to publish the rest of a partially bootstrapped group.

Node and Web use the same report selector in `scripts/release_attempt_history.py`. It may select
the nearest valid older report only when every intervening upload is proven not to have run;
preserve the selected report's actual run/attempt and manifest identity. Missing, malformed or
ambiguous history remains observation-only. The shared `scripts/npm_package_group.py` reconciler
atomically persists upload intent and recovery state before subprocesses can lose them. A dated
Windows incident decision is not a general resend override.

For Python wheel assets, rerun the original run's failed upload job with its retained wheels.
It compares all existing same-name assets before sending missing ones, then compares the
complete anonymous public downloads. Preserve that successful GitHub handoff before resuming
the dependent PyPI job; do not rebuild wheels to repair a partial upload.

For Flutter with unchanged facade source, retain the frozen thin facade, its
source/tree/version/digest receipt and all manifest-pinned native assets. Rerun only the original
run's failed upload/publication jobs. Main-line native assets belong to the existing non-draft
`v<version>` workspace Release. Matching assets are kept; different bytes stop recovery. Reconcile
every existing pub.dev file's path, executable bits, size and bytes before writes, and require
anonymous verification of all eleven native target downloads before OIDC. An exact accepted
facade needs no new upload; a conflicting accepted package cannot be repaired in place. If an
earlier authentication-tag run or attempt has an unknown upload outcome, continue observation of that frozen package. Registry
absence does not authorize an automatic resend or a fresh run that bypasses the history.

For a facade-only repair before that facade is accepted, use Tag And Push's dispatch recipe:
keep `--ref` at the immutable `flutter-v<version>` authentication tag and `release_tag` at the
workspace `v<version>` label. Verify the optional pub.dev dispatch event is enabled, set
`source_ref` to the newer reviewed full 40-character facade commit, and pass `native_run_id` for
the original completed `release-flutter.yml` or `release-preflight.yml` run. Require exactly one unexpired immutable
`merman-flutter-native-assets` artifact; a successful preflight uses the same artifact name and
schema. The native manifest's source must still match the workspace tag. The read-only builder
reuses those assets and the unchanged manifest, then repackages, analyzes, dry-runs and qualifies
the facade against them without native cross-builds. Full consumer-contract checks remain
required. The schema-1 archive receipt identifies facade source/tree, the schema-1 manifest
identifies native source/tree, and their versions agree; the archive digest binds the included
manifest. Preserve historical `flutter-v<version>` downloads and recover 0.9.x through its original
tagged workflow without changing accepted artifacts. The existing `flutter-v{{version}}` publisher
pattern remains valid for both lines; no maintenance configuration switch is needed. See
[Required Credentials](../../../../docs/release/RELEASING.md#required-credentials) only for the
additional event setting when selecting manual publication recovery.

Acquire registry credentials only at the actual upload boundary. Workspace Cargo owners mint a
fresh temporary token for each upload after batch dry runs and revoke it afterwards; single-crate
owners use the official authentication action immediately before upload. Follow current workflow
implementations and their focused protocol/security tests. Authentication failure before upload
is distinct from uncertain upload acceptance; preserve that distinction without automatic fallback.

Done: accepted versions match the frozen evidence, remaining sends qualify through their recovery owner, and
every unresolved acceptance or mismatch retains its original report and next observation condition.

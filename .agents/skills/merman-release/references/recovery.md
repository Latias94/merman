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
members directly under the final tag. The current package-group operator does not repair
dist-tags; resolve a tag conflict through a separately authorized operation after checking the
current registry permissions. Verify registry
provenance rather than treating an embedded provenance file as an attestation.
For first-publication bootstrap, follow Required Credentials' same-artifact procedure. A
nonpublishing candidate has no upload history report; cross-run observation-only recovery
must not be used to publish the rest of a partially bootstrapped group.

For Python wheel assets, rerun the original run's failed upload job with its retained wheels.
It compares all existing same-name assets before sending missing ones, then compares the
complete anonymous public downloads. Preserve that successful GitHub handoff before resuming
the dependent PyPI job; do not rebuild wheels to repair a partial upload.

For Flutter, retain the frozen thin facade, its source/tree/version/digest receipt and all
manifest-pinned native assets from the original run. Rerun failed upload/publication jobs from
that run rather than rebuilding. If the Flutter-tagged GitHub Release is missing, create it
within the authorized release scope before retrying the native-asset job. Matching existing
native assets are kept; different bytes stop recovery. Require anonymous verification of every
native download before publishing the facade. Use the trusted extractor and archive verifier
before configuring OIDC. Reconcile pub.dev paths, executable bits, sizes and bytes; a workflow
upload success alone does not establish byte identity.

Acquire registry credentials only at the actual upload boundary. Workspace Cargo owners mint a
fresh temporary token for each upload after batch dry runs and revoke it afterwards; single-crate
owners use the official authentication action immediately before upload. Follow current workflow
implementations and their focused protocol/security tests. Authentication failure before upload
is distinct from uncertain upload acceptance; preserve that distinction without automatic fallback.

Done: accepted versions match the frozen evidence, remaining sends are proven unattempted, and
every unresolved acceptance or mismatch retains its original report and next observation condition.

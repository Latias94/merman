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
members directly under the final tag. Trusted Publishing does not authorize a later `dist-tag`
repair; a tag conflict needs a separately authorized maintainer credential. Verify registry
provenance rather than treating an embedded provenance file as an attestation.

For Flutter, retain the frozen native package archive with its source/tree/version/digest receipt.
Use the trusted extractor and archive verifier before configuring OIDC. Reconcile registry paths,
executable bits, sizes and bytes; a workflow upload success alone does not establish byte identity.

Acquire registry credentials only at the actual upload boundary. Workspace Cargo owners mint a
fresh temporary token for each upload after batch dry runs and revoke it afterwards; single-crate
owners use the official authentication action immediately before upload. Follow current workflow
implementations and their focused protocol/security tests. Authentication failure before upload
is distinct from uncertain upload acceptance; preserve that distinction without automatic fallback.

Done: accepted versions match the frozen evidence, remaining sends are proven unattempted, and
every unresolved acceptance or mismatch retains its original report and next observation condition.

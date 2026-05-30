# Workspace Release Versioning — Milestones

Status: Closed
Last updated: 2026-05-30

## M0 — Package Graph Frozen

Exit criteria:

- Package blockers are recorded with exact commands.
- The lane explicitly treats FFI/UniFFI ABI work as complete.

Status: complete. Current blockers are crates.io dependency availability and immutable `0.6.0`
versions, not missing FFI ABI work.

## M1 — Publish Order And Version Decision

Exit criteria:

- The next workspace version is selected.
- Publish order is documented in dependency order.
- The doc distinguishes publish preparation from actual `cargo publish`.

Status: complete. `docs/release/PUBLISH_ORDER.md` selects `0.7.0`, documents full workspace and
binding-specific publish order, and keeps `cargo publish` out of the lane.

## M2 — Version Alignment

Exit criteria:

- Workspace version and internal dependency version requirements agree.
- Binding crates still compile after version alignment.

Status: complete. Workspace package version, workspace dependency entries, and explicit binding
facade dependencies are aligned to `0.7.0`; `merman-ffi` and `merman-uniffi` both compile.

## M3 — Package Gate Matrix

Exit criteria:

- Package file-list checks are recorded.
- Full package verification results are recorded where possible.
- Remaining crates.io-only blockers are explicit.

Status: complete. `docs/release/PUBLISH_ORDER.md` records pass/blocker state in dependency order.
The remaining blockers are expected crates.io availability blockers for unpublished `0.7.0`
workspace crates.

## M4 — Closeout

Exit criteria:

- Focused checks pass.
- Release blockers are closed or split.
- Platform lanes can proceed from a documented baseline.

Status: complete. `0.7.0` is the documented baseline; platform lanes can proceed, and publishing
must follow `docs/release/PUBLISH_ORDER.md`.

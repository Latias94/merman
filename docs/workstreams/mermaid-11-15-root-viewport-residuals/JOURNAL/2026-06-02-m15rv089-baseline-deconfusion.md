# M15RV-089 - Baseline Deconfusion And Override Inventory Recheck

Date: 2026-06-02
Task: M15RV-089

## Summary

Cleaned up stale baseline wording in the active parity tooling and re-ran the override inventory so
the 11.15 lane stops talking about itself as if it were still pinned to 11.12.x.

## Why

This is not cosmetic churn. The remaining parity work is increasingly about deciding which
browser-derived facts should stay as explicit headless approximations and which should be replaced
by source-derived logic. That decision becomes noisy and self-deceptive if the repo keeps mixing:

- real 11.15 baseline semantics,
- historical generated filenames like `*_11_12_2.rs`,
- and stale report headers that still print `11.12.3`.

Before pruning more override debt, the reporting surface needs to tell the truth.

## Scope

- Reused the pinned-baseline lookup from `REPOS.lock.json` in xtask override reporting.
- Reused the same helper in root override audit report generation.
- Added an explicit note in `crates/merman-render/src/generated/mod.rs` that the
  `11_12_2` suffix is historical naming debt, not the active semantic contract.
- Removed the stale `mermaid@11.12.3` top-level crate doc claim in `merman-core`.
- Re-ran `report-overrides` to get a fresh 11.15 inventory snapshot.

## Verification

- `cargo test -p xtask overrides::report -- --nocapture`
- `cargo test -p xtask root_override_audit -- --nocapture`
- `cargo run -p xtask -- report-overrides`

## Current inventory snapshot

- Root viewport overrides: `241`
- Text lookup overrides: `488`
- Sequence SVG text rows: `1036`
- Flowchart font metric rows: `3774`

## Notes

This slice intentionally does not rename generated files. A mass rename would create broad churn
without improving renderer truth. The right next step is to use this clearer inventory to choose
which override families should shrink, and which historical names can later be migrated in one
controlled regeneration pass.

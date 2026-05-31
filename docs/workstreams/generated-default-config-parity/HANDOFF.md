# Generated Default Config Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The workstream has been opened from the Mermaid 11.15 closeout concern. GDC-020 split
`xtask verify-generated` into artifact-specific checks. GDC-030 made `verify-default-config` green
through an explicit override manifest applied by `gen-default-config`. GDC-040 updated DOMPurify to
Mermaid 11.15's resolved `dompurify@3.4.0` baseline and made `verify-generated` green.

## Active Task

- Task ID: GDC-040
- Owner: codex
- Files: `crates/xtask/src/cmd/generate.rs`, `crates/xtask/src/cmd/snapshots.rs`,
  `crates/merman-core/src/generated/dompurify_defaults.rs`, `tools/upstreams/REPOS.lock.json`,
  `docs/adr/0024-dompurify-default-allowlists-and-generation.md`
- Validation: `cargo run -p xtask -- verify-dompurify-defaults`;
  `cargo run -p xtask -- verify-generated`; `cargo nextest run -p xtask`;
  `cargo nextest run -p merman-core`; `cargo fmt --check`; `git diff --check`
- Status: DONE
- Review: pending
- Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Keep `verify-generated` as an umbrella command for compatibility.
- Add artifact-specific commands before changing default config generation semantics.
- Aggregate `verify-generated` failures across artifact families so a missing optional checkout does
  not hide a separate artifact mismatch.
- Use `crates/xtask/default_config_overrides.json` as the reviewed default-config override manifest.
- `gen-default-config` applies the manifest by default; `--no-local-overrides` keeps schema-only
  output available for diagnosis.
- DOMPurify remains part of `verify-generated`; the required reference checkout is pinned in
  `tools/upstreams/REPOS.lock.json`.
- Missing default DOMPurify reference material now returns an actionable `MissingReference` error
  instead of a bare file-read failure.

## Concerns

- `repo-ref/dompurify` is local reference material and is not committed. Fresh environments must
  materialize it at the lockfile ref before running `verify-dompurify-defaults` or `verify-generated`.

## Next Recommended Action

- Continue to GDC-050 to close the lane, or split a follow-on for Pie 11.15 config knobs.

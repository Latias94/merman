# Generated Default Config Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The workstream has been opened from the Mermaid 11.15 closeout concern. GDC-020 split
`xtask verify-generated` into artifact-specific checks. GDC-030 made `verify-default-config` green
through an explicit override manifest applied by `gen-default-config`.

## Active Task

- Task ID: GDC-030
- Owner: codex
- Files: `crates/xtask/src/cmd/generate.rs`, `crates/xtask/default_config_overrides.json`,
  `docs/adr/0019-generated-default-config.md`
- Validation: `cargo run -p xtask -- verify-default-config`; `cargo nextest run -p xtask`;
  `cargo nextest run -p merman-core config`; `cargo nextest run -p merman-render`;
  `cargo fmt --check`; `git diff --check`
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

## Concerns

- `verify-dompurify-defaults` is red when `repo-ref/dompurify/dist/purify.cjs.js` is absent; GDC-040
  should decide whether that remains an optional reference-checkout gate or gets a bootstrap path.

## Next Recommended Action

- Continue to GDC-040 to clarify DOMPurify reference checkout policy and make the umbrella generated
  artifact gate's remaining failure actionable.

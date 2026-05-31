# Generated Default Config Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The workstream has been opened from the Mermaid 11.15 closeout concern. GDC-020 split
`xtask verify-generated` into artifact-specific checks so default config drift can be diagnosed
without requiring a DOMPurify checkout.

## Active Task

- Task ID: GDC-020
- Owner: codex
- Files: `crates/xtask/src/main.rs`, `crates/xtask/src/cmd/snapshots.rs`, ADR/rendering docs
- Validation: `cargo nextest run -p xtask`; `cargo run -p xtask -- verify-default-config`;
  `cargo run -p xtask -- verify-dompurify-defaults`; `cargo fmt --check`; `git diff --check`
- Status: DONE_WITH_CONCERNS
- Review: pending
- Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Keep `verify-generated` as an umbrella command for compatibility.
- Add artifact-specific commands before changing default config generation semantics.
- Aggregate `verify-generated` failures across artifact families so a missing optional checkout does
  not hide a separate artifact mismatch.
- Treat defaultConfig overlay modeling as GDC-030.

## Concerns

- `verify-default-config` is now independently red with `default config mismatch`; GDC-030 must make
  the generator match the committed Mermaid 11.15 default artifact.
- `verify-dompurify-defaults` is red when `repo-ref/dompurify/dist/purify.cjs.js` is absent; GDC-040
  should decide whether that remains an optional reference-checkout gate or gets a bootstrap path.

## Next Recommended Action

- Continue to GDC-030 to make `verify-default-config` green for Mermaid 11.15.

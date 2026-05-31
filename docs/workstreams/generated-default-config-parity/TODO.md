# Generated Default Config Parity - TODO

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

- [x] GDC-010 [owner=planner] [deps=none] [scope=docs/workstreams/generated-default-config-parity]
  Goal: Freeze the problem, target state, non-goals, and first implementation slice.
  Validation: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and
  `CONTEXT.jsonl` exist and agree.
  Evidence: `docs/workstreams/generated-default-config-parity/DESIGN.md`
  Context: `docs/workstreams/generated-default-config-parity/CONTEXT.jsonl`
  Handoff: DONE. The lane starts from the Mermaid 11.15 closeout concern that generated default
  config and DOMPurify verification are coupled under `verify-generated`.

## M1 - Artifact-Specific Verification Surface

- [x] GDC-020 [owner=codex] [deps=GDC-010] [scope=crates/xtask/src/main.rs,crates/xtask/src/cmd/snapshots.rs,docs/adr,docs/rendering]
  Goal: Split generated artifact verification into default-config and DOMPurify commands while
  preserving `verify-generated` as the umbrella command.
  Validation: `cargo nextest run -p xtask`; `cargo run -p xtask -- verify-default-config`;
  `cargo run -p xtask -- verify-dompurify-defaults`; `cargo fmt --check`; `git diff --check`.
  Review: Confirm each command owns only one artifact family and that `verify-generated` still
  delegates to the split checks.
  Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`
  Context: `docs/workstreams/generated-default-config-parity/CONTEXT.jsonl`
  Handoff: DONE_WITH_CONCERNS. Added `verify-default-config` and `verify-dompurify-defaults`,
  kept `verify-generated` as an umbrella command, and made umbrella failures aggregate across
  artifact families. The split is complete, but `verify-default-config` is independently red
  because the generator still lacks Mermaid `defaultConfig.ts` overlay semantics; that is GDC-030.

## M2 - Default Config Overlay Parity

- [x] GDC-030 [owner=codex] [deps=GDC-020] [scope=crates/xtask/src/cmd/generate.rs,crates/merman-core/src/generated/default_config.json,docs/adr/0019-generated-default-config.md]
  Goal: Make `verify-default-config` green by modeling Mermaid 11.15 `defaultConfig.ts` overlay
  semantics or by introducing an explicit generated override manifest.
  Validation: `cargo run -p xtask -- verify-default-config`; `cargo nextest run -p merman-core config`;
  `cargo nextest run -p merman-render`; `cargo fmt --check`.
  Review: Confirm schema defaults, overlay defaults, and intentionally unsupported diagram family
  defaults are clearly separated.
  Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`
  Context: `docs/workstreams/generated-default-config-parity/CONTEXT.jsonl` plus ADR-0019.
  Handoff: DONE. Added `crates/xtask/default_config_overrides.json`, made `gen-default-config`
  apply it by default, kept `--no-local-overrides` for schema-only diagnostics, and proved
  `verify-default-config` green. The manifest separates upstream non-JSON/defaultConfig behavior,
  local parity overrides, deferred/out-of-scope families, and unsupported Pie 11.15 config knobs.

- [x] GDC-040 [owner=codex] [deps=GDC-020] [scope=docs/adr/0024-dompurify-default-allowlists-and-generation.md,crates/xtask/src/cmd/generate.rs]
  Goal: Clarify DOMPurify source checkout policy for Mermaid 11.15 and decide whether the verifier
  should fail with remediation text or be treated as an optional bootstrap gate.
  Validation: `cargo run -p xtask -- verify-dompurify-defaults` with the expected reference checkout
  state documented; `cargo fmt --check`.
  Review: Confirm the command does not hide sanitizer drift and does not require committed external
  dist files.
  Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`
  Context: ADR-0024 and this workstream context.
  Handoff: DONE. DOMPurify remains part of the umbrella `verify-generated` gate. The baseline now
  follows Mermaid 11.15's resolved `dompurify@3.4.0`; `repo-ref/dompurify` is required reference
  material and missing checkouts report actionable remediation text.

## M3 - Closeout

- [ ] GDC-050 [owner=planner] [deps=GDC-030,GDC-040] [scope=docs/workstreams/generated-default-config-parity,docs/releasing]
  Goal: Close the lane or split remaining generated-artifact verification debt.
  Validation: Fresh closeout gates recorded in `EVIDENCE_AND_GATES.md`.
  Review: Run workstream review and fresh verification before marking complete.
  Evidence: `docs/workstreams/generated-default-config-parity/EVIDENCE_AND_GATES.md`
  Context: this workstream.
  Handoff: Summarize residual risks and next lane candidates.

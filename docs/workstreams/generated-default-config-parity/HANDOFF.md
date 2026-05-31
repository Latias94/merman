# Generated Default Config Parity - Handoff

Status: Closed
Last updated: 2026-05-31

## Current State

The workstream was opened from the Mermaid 11.15 closeout concern. GDC-020 split
`xtask verify-generated` into artifact-specific checks. GDC-030 made `verify-default-config` green
through an explicit override manifest applied by `gen-default-config`. GDC-040 updated DOMPurify to
Mermaid 11.15's resolved `dompurify@3.4.0` baseline and made `verify-generated` green. GDC-050
reviewed the lane, ran fresh closeout gates, and closed the workstream.

## Active Task

- Task ID: none
- Owner: codex
- Files: `docs/workstreams/generated-default-config-parity/*`, `docs/rendering/REFACTOR_TODO.md`,
  `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
- Validation: `cargo nextest run --workspace`; `cargo run -p xtask -- verify-generated`;
  `cargo run -p xtask -- verify-default-config`; `cargo run -p xtask -- verify-dompurify-defaults`;
  `cargo fmt --check`; `git diff --check`
- Status: CLOSED
- Review: no blocking workstream or code-quality findings; one stale rendering TODO status was fixed
  during closeout.
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
- Close the generated-artifact verification lane rather than keeping it open for new diagram family
  support. Pie 11.15 behavior and deferred diagram families are separate product/parity scopes.

## Concerns

- `repo-ref/dompurify` is local reference material and is not committed. Fresh environments must
  materialize it at the lockfile ref before running `verify-dompurify-defaults` or `verify-generated`.
- Mermaid 11.15 Pie config keys (`donutHole`, `highlightSlice`, `legendPosition`) remain explicit
  follow-ons because the current renderer does not implement those behaviors.
- Deferred 11.15 diagram families remain outside this lane.

## Next Recommended Action

- Open a focused Pie 11.15 parity lane for `donutHole`, `highlightSlice`, and `legendPosition`, or
  choose one deferred 11.15 diagram family lane if product coverage is the priority.

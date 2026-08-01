# Generated Default Config Parity - Handoff

Status: Closed
Last updated: 2026-05-31

> Superseded on 2026-07-16 by ADR-0019's Mermaid 11.16 three-plane projection. References below to
> the deleted override manifest describe the closed 11.15 implementation and are not current
> instructions.

## Current State

The workstream established artifact-specific verification during Mermaid 11.15. Its override
manifest implementation has since been deleted. The current Mermaid 11.16 implementation projects
the content-pinned runtime into separate JSON-value and key-shape artifacts, then applies a separate
typed Merman security policy in memory. ADR-0019 owns the current contract.

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
- Generate the upstream JSON value and config-key shape together with `gen-default-config`.
- Use the content-pinned Mermaid runtime as the only generation authority; do not maintain a second
  Rust replay of `defaultConfig.ts`.
- Keep Merman's hardened secure list out of the upstream value artifact.
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

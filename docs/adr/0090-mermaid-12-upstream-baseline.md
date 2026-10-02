# ADR-0090: Mermaid 12 Upstream Baseline

## Status

Accepted for `0.8.0-alpha.7`. Supersedes ADR-0001 for active compatibility, fixture admission,
and release-facing documentation. Mermaid 11 references retained by older ADRs and progress records
are historical evidence.

## Date

2026-09-24

## Context

The Mermaid 11 baseline in ADR-0001 no longer describes the selected implementation contract. The
Mermaid 12 alignment work has migrated the reference bundle, generated projections, legal materials,
product defaults, and the primary upstream SVG corpus to one reproducible source identity. Keeping the
old baseline active would make the compatibility dashboard, generated artifacts, and fixture
provenance disagree.

## Decision

- Active Mermaid source: `mermaid@12.0.0` at commit
  `98a0945418c76238f15df2afaddbba4272656c3b`.
- Reference CLI: `@mermaid-js/mermaid-cli@11.17.0`.
- Sanitizer companion: `dompurify@3.4.16` after the GHSA-p98j-92pf-mc4p security patch.
  Alpha.7 originally selected 3.4.15; sanitizer-specific decisions are recorded in ADR-0024.
- Upstream SVG manifests use renderer revision `xtask-upstream-svg-v5` and schema 4, which bind
  the built reference artifact, its compiler, and the sanitizer actually called by Mermaid.
  Alpha.7 used revision `xtask-upstream-svg-v4`. The 37 admitted family manifests share the active
  Mermaid 12 identity; browser-specific measurements remain evidence in those manifests rather
  than product semantics.
- `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json` is the authority for generated Rust and
  Playground projections. `docs/release/THIRD_PARTY_COMPONENTS.json` and generated notices remain the
  authorities for artifact-specific legal closures.
- Mermaid 11 fixtures and source references are preserved only when a document or regression test
  explicitly labels them historical. They do not define current compatibility claims.

## Consequences

Active alignment documents, compatibility README text, generated notices, and release audit headers
refer to Mermaid 12. The source-backed implementation and fixture review still own individual parity
residuals; changing the baseline does not waive a failed family gate or turn browser measurements into
semantic acceptance.

Future Mermaid baseline changes must add a successor ADR and update the bundle, generated
projections, manifest provenance, legal materials, and active compatibility documents together.
Companion security patches retain the Mermaid baseline and update their existing owner ADR and
selection receipt, with the same generated-artifact, provenance, and legal checks.

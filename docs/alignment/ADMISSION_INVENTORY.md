# Diagram Admission Inventory

Status: Active
Prior baseline evidence: Mermaid `@12.0.0` at `98a0945418c76238f15df2afaddbba4272656c3b`
Last reviewed: 2026-10-02

The current unreleased transition targets Mermaid `@12.1.0` at `21f72f07ea22c0af48a3149c550654e80d8e40cb`; the primary matrix and generated oracle evidence remain bound to the preceding `12.0.0` selection until the reference refresh is admitted.

The admission consistency checks live in `crates/xtask/src/cmd/admission.rs`.

Admission is composed from three owner-local structures:

- `merman-core` family capabilities own the canonical public family set and parser/render facts;
- `DIAGRAM_VERIFICATION_FACTS` in `crates/xtask/src/cmd/compare/diagrams.rs` owns the 37-family
  primary SVG matrix, compare commands, default DOM modes, and family-specific compare policy; and
- per-family fixture directories, semantic/layout goldens, and upstream
  `_baseline-manifest.json` files own admitted evidence bytes and provenance.

`zenuml` is part of the canonical family capability set but not `DIAGRAM_VERIFICATION_FACTS`. Its
external plugin comparison therefore remains a separate compatibility lane rather than a built-in
upstream-SVG matrix claim.

`admission.rs` cross-checks those owners; it does not duplicate them into another registry. Family
alignment documents explain the resulting contract for humans, but their paths and wording are
not machine admission inputs.

Current consumers:

- `xtask compare-all-svgs` reads the 37-family primary SVG matrix directly from the compare facts;
  all current primary families participate in the root-validation contract.
- Per-diagram `xtask compare-*` commands keep their CLI adapters, but shared fixture discovery,
  upstream/local SVG loading, DOM checks, local SVG output writing, and result sections live in the
  compare harness. Diagram adapters own only render-specific policy such as marker checks,
  root/label delta rows, ELK admission, or family-specific skip decisions.
- `xtask check-alignment` verifies canonical family capabilities, required fixture and
  semantic/layout golden presence, upstream SVG manifests, and compare-command presence for
  primary diagrams.
- Default-config parity is orthogonal to diagram admission. `xtask verify-default-config`
  regenerates the upstream value and key-shape artifacts from the selected content-pinned Mermaid
  runtime. No admission-specific override manifest can remove a family or key.
- `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` uses the primary SVG matrix as the boundary for
  rendered config claims: accepted/merged config can be broader than primary SVG support, but
  rendered support should point at an admitted family test, golden, or an explicit residual.

This overview does not move fixtures or weaken evidence by itself. The historical Mermaid 11.17.2
admission process and the rules for future baseline additions are recorded in
`docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`.

No normal verification command parses this document. See [`README.md`](README.md) for the
alignment authority map.

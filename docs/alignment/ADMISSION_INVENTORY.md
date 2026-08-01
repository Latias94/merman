# Diagram Admission Inventory

Status: Active
Baseline: Mermaid `@11.16.0`
Last updated: 2026-07-19

The structured admission inventory lives in `crates/xtask/src/cmd/admission.rs`.

The executable inventory has two family-level states:

- `PrimarySvgMatrix`: 35 families with semantic/layout goldens, typed rendering, pinned upstream
  SVGs, and executable compare facts;
- `CompatibilityOnly`: `zenuml`, whose complete local grammar, semantic/editor model, typed layout,
  and headless SVG are aligned to the admitted ZenUML Core behavior source, but whose external
  browser renderer is not represented as a primary upstream-SVG parity claim.

Every inventory record owns:

- a normalized fixture corpus, optionally with a deferred investigation corpus;
- semantic, layout, SVG baseline, and root viewport coverage;
- compare command ownership;
- owning alignment document;
- an explicit reason for any deferred coverage dimension.

Parser and typed-render capability evidence is projected from `merman-core` diagram family facts.
The inventory still owns fixture corpus state, coverage status, compare-command ownership, owner
docs, and defer reasons because those are release/admission policy rather than parser registry
facts.

Current consumers:

- `xtask compare-all-svgs` reads the 35-family primary SVG matrix projection from the inventory;
  every current primary record has covered root-viewport evidence.
- Per-diagram `xtask compare-*` commands keep their CLI adapters, but shared fixture discovery,
  upstream/local SVG loading, DOM checks, local SVG output writing, and result sections live in the
  compare harness. Diagram adapters own only render-specific policy such as marker checks,
  root/label delta rows, ELK admission, or family-specific skip decisions.
- `xtask check-alignment` verifies inventory paths, owner docs, semantic/layout fixture evidence,
  upstream SVG directories, compare-command presence for primary diagrams, and reasons for
  deferred coverage.
- `xtask check-alignment` also checks that semantic/layout/SVG-covered records are backed by the
  corresponding `merman-core` family capability facts.
- Default-config parity is orthogonal to diagram admission. `xtask verify-default-config`
  regenerates the upstream value and key-shape artifacts from the content-pinned Mermaid 11.16
  runtime. No admission-specific override manifest can remove a family or key.
- `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` uses this inventory as the admission boundary for
  rendered config claims: accepted/merged config can be broader than primary SVG support, but
  rendered support should point at an admitted family test, golden, or an explicit residual.

This inventory does not move fixtures or weaken evidence by itself. The completed Mermaid 11.16
admission process and the rules for future baseline additions are recorded in
`docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`.

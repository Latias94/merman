# Diagram Admission Inventory

Status: Active
Selected reference: Mermaid `@12.3.0` at `2086904e76c692331e34a45d7cc3bc30860d040e`
Last reviewed: 2026-11-04

The 37-family primary matrix and its upstream SVG manifests select Mermaid `@12.3.0`. A pinned reference and refreshed evidence do not by themselves certify the strict parity gates on the final release commit; retain their run-specific results separately.

The admission consistency checks live in
[`admission.rs`](../../crates/xtask/src/cmd/admission.rs).

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

## Evidence Boundaries

An admitted input needs the evidence required by its family capability and comparison policy:
canonical semantics, typed layout where available, selected-runtime SVG provenance, and an
executable compare path. Required layers must agree on the input bytes. A parser-only input,
explicit manifest exclusion, raw upstream corpus, or controlled family-local experiment does not
become SVG admission merely by existing in the repository.

The per-family manifests own exact included and excluded inputs; compare facts own exceptional
parsing, tooling skips, and structure-only cases. Neither historical fixture counts nor file-name
patterns may broaden those exceptions. The
[family notes](FAMILY_NOTES.md) explain the remaining human-facing caveats without changing
acceptance.

For a new family or source transition, use the existing capability owner, compare facts, and
normal fixture import/baseline machinery. The
[upgrade playbook](../release/MERMAID_UPGRADE_PLAYBOOK.md) owns source-graph selection and review.
Add a focused test or input for an uncovered source branch or reported failure; do not create a
parallel prose registry or treat a prior release's green result as current evidence.

No normal verification command parses this document. See [`README.md`](README.md) for the
alignment authority map.

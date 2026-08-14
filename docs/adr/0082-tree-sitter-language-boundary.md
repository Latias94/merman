# ADR-0082: Tree-sitter Mermaid Language Boundary

## Status

Accepted

## Dates

- Accepted: 2026-08-14

## Context

Merman has a complete strict Mermaid parser portfolio, typed semantic models, recovery journals,
analysis facts, editor behavior, and LSP operations for the pinned Mermaid baseline. Those parsers
reproduce behavior that a concrete syntax tree does not own: DB mutation order, validation,
defaults, diagnostics, navigation identity, refactoring safety, IR, and render semantics.

Tree-sitter solves a different problem. Editors and language tools benefit from a tolerant,
incremental concrete syntax tree and portable queries that remain useful while a buffer is
incomplete. Existing Mermaid grammars provide useful ecosystem evidence, but none owns a
source-backed contract for all 35 public Merman families or the exact Mermaid and ZenUML baselines.

Treating Tree-sitter as a replacement parser would create an unproved second semantic engine.
Treating it as an editor-internal parser would conflict with ADR-0071, which prohibits successful
editor semantics that are not produced by the family semantic construction. A separately
distributed syntax product can exist without violating either boundary if ownership and data flow
remain explicit.

## Decision

Merman maintains an independently versioned Tree-sitter Mermaid language source package under
`distribution/tree-sitter-mermaid/`. It is an external tolerant CST and query product, not part of
Merman's semantic parser stack.

The following ownership rules are mandatory:

- Mermaid and the selected ZenUML companion own accepted source syntax.
- `merman-core` remains the only owner of strict validity, semantic construction, DB mutation
  order, IR, diagnostics, navigation identity, and refactoring safety.
- The Tree-sitter package owns named CST nodes and fields, family root nodes, recovery nodes,
  portable queries, editor query adapters, generated parsers, bindings, WASM, and package metadata.
- Merman's public family IDs, internal variants, and authoring-header suggestions are projected
  read-only from the existing family catalog. Complete accepted syntax remains source-backed by the
  pinned Mermaid and ZenUML authorities. The package stores only foreign-key IDs plus its own roots,
  maturity, evidence, and query applicability. A deterministic composed contract binds both
  authority digests.
- The conformance harness is one-way: Merman may classify strict-valid fixtures and expected public
  families, then inspect Tree-sitter output. No Merman production crate may consume Tree-sitter CST
  nodes as semantic facts or depend on Tree-sitter.
- Tree-sitter recovery is useful editing behavior, not proof of Mermaid validity. A tree without an
  `ERROR` node does not imply strict validity.

Support claims use an executable monotonic lattice: `recognized`, `structured`, `query-complete`,
and `conformant`. A separate `planned` lifecycle has no support tier. Internal variants and aliases
do not increase the 35-family count. Generic body fallbacks may be used only during construction,
remain visible as below `structured`, and must be deleted before full conformance.

The package pins each consumer runtime independently: Tree-sitter CLI, Rust runtime, and web
runtime `0.26.12`; source-built Node runtime `0.25.1`; generated language ABI 14. Package version,
CST schema version, and query schema version are independent axes. Publication is blocked until a
separate registry-ownership decision; package assembly and release workflow paths remain dry-run
only in the meantime.

Existing semantic parser code may be deleted only under a later superseding ADR with strict parse,
recovery, mutation-order, IR, render, analysis, and editor equivalence evidence. This decision does
not authorize deleting LALRPOP or handwritten family parsers.

## Consequences

- Merman can offer a first-class Tree-sitter ecosystem surface without destabilizing rendering,
  analysis, editor-core, or LSP behavior.
- Grammar and query changes have a focused CI owner, while shared catalog, workspace, and baseline
  changes still run all affected owners.
- The repository pays for generated C/WASM size, binding tests, corpus tests, fuzzing, and package
  maintenance even though production Merman artifacts remain Tree-sitter-free.
- Pre-1.0 CST and capture changes may break consumers, but schema receipts and migration notes make
  those changes explicit.
- Mermaid or ZenUML baseline movement does not silently rewrite historical package claims. Package
  support and repository alignment are separate machine-readable states.

## Rejected Alternatives

### Replace Merman's semantic parsers with Tree-sitter

Rejected. Concrete syntax and incremental recovery do not replace semantic construction, parser
side effects, typed models, diagnostics, or refactoring identity. There is no equivalence evidence.

### Feed Tree-sitter CST facts into editor-core or LSP

Rejected. That would recreate the second successful editor grammar removed by ADR-0071 and permit
semantic disagreement with rendering.

### Keep the language package in a separate repository

Rejected for initial development. Atomic review with the pinned Mermaid source, family catalog,
fixtures, legal inventory, and one-way oracle is more valuable until the conformance contract is
stable.

### Claim coverage through a generic line fallback

Rejected. Recognition without useful named structure is represented honestly as `recognized`, not
`structured` or `conformant`.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0071: Editor-Facing Parser and Semantic Seam
- ADR-0073: Family-Owned Diagram Architecture
- ADR-0075: ZenUML Parser Technology
- ADR-0076: Capability-Driven Feature and Package Surfaces
- ADR-0081: Release Quality Gates

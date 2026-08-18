# ADR-0083: Tree-sitter Mermaid Language Boundary

## Status

Accepted; production highlighting boundary amended by ADR-0084.

## Dates

- Accepted: 2026-08-14
- Amended: 2026-08-16
- Amended: 2026-08-17 by ADR-0084

## 2026-08-17 Highlighting Amendment

ADR-0084 makes the canonical Tree-sitter highlight query the sole syntax-coloring implementation
for Merman's native LSP and Playground. It narrows this ADR's blanket production-dependency
prohibition only for the exact `lsp-library` and `lsp-stdio-release` artifact profiles, and narrows
the rejected LSP CST use to semantic use. Core, analysis, editor-core, render, IR, Web and
WebAssembly, CLI, and language-binding artifact closures remain Tree-sitter-free.

The strict semantic boundary remains unchanged: Tree-sitter recovery is not proof of Mermaid
validity and cannot own diagnostics, completion, identity, navigation, rename, IR, or rendering.
The one-way conformance strategy, independent distribution, and lean verification policy also
remain in force.

## Context

Merman has a complete strict Mermaid parser portfolio, typed semantic models, recovery journals,
analysis facts, editor behavior, and LSP operations for the pinned Mermaid baseline. Those parsers
reproduce behavior that a concrete syntax tree does not own: DB mutation order, validation,
defaults, diagnostics, navigation identity, refactoring safety, IR, and render semantics.

Tree-sitter solves a different problem. Editors and language tools benefit from a tolerant,
incremental concrete syntax tree and portable queries that remain useful while a buffer is
incomplete. Existing Mermaid grammars provide useful ecosystem evidence, but none owns a
source-backed contract for all 35 public Merman families or the exact Mermaid and ZenUML baselines.

Treating Tree-sitter as a replacement parser would create an unproved second semantic engine. The
original decision also kept it out of editor-internal production paths to avoid successful editor
semantics that were not produced by the family semantic construction. ADR-0084 later admitted the
tolerant CST for syntax highlighting while preserving that semantic prohibition.

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
- Merman's public family IDs and strict-valid fixture corpus are projected read-only through one
  Rust integration test. Complete accepted syntax remains source-backed by the pinned Mermaid and
  ZenUML authorities; the Tree-sitter package does not define another semantic contract.
- The conformance harness is one-way: Merman may classify strict-valid fixtures and expected public
  families, then inspect Tree-sitter output. No Merman production crate may consume Tree-sitter CST
  nodes as semantic facts. ADR-0084 permits only `merman-lsp` library and stdio artifacts to depend
  on Tree-sitter for syntax highlighting; other Merman production crates remain dependency-free.
- Tree-sitter recovery is useful editing behavior, not proof of Mermaid validity. A tree without an
  `ERROR` node does not imply strict validity.

The standard Tree-sitter corpus owns CST/recovery expectations. A single strict-valid fixture
oracle, focused incremental/scanner tests, query compilation, and one representative consumer smoke
per binding are the complete maintenance boundary. The package must not grow a second test engine,
support-tier lattice, receipt graph, duplicated schema snapshot, or per-editor capture matrix.
Generic body fallbacks may be used only during construction and must be deleted before a family is
claimed as structured.

The package pins each consumer runtime independently: Tree-sitter CLI, Rust runtime, and web
runtime `0.26.12`; native Node runtime `0.25.x`; generated language ABI 15. It is publishable as
`tree-sitter-mermaid` on crates.io and npm and as a GitHub source/WASM release, subject to protected
release authorization. Browser consumers use the external `web-tree-sitter` runtime and the
package's root language WASM; the package does not add a grammar-specific TypeScript runtime.

Existing semantic parser code may be deleted only under a later superseding ADR with strict parse,
recovery, mutation-order, IR, render, analysis, and editor equivalence evidence. This decision does
not authorize deleting LALRPOP or handwritten family parsers.

## Consequences

- Merman can offer a first-class Tree-sitter ecosystem surface without destabilizing rendering,
  analysis, editor-core, or LSP behavior.
- Grammar and query changes have a focused CI owner, while shared catalog, workspace, and baseline
  changes still run all affected owners.
- The repository pays for generated C/WASM size, binding tests, corpus tests, fuzzing, and package
  maintenance. LSP library and stdio artifacts additionally pay the native Tree-sitter runtime
  cost; other Merman production artifacts remain Tree-sitter-free.
- Pre-1.0 CST and capture changes may break consumers; semver and migration notes make those
  changes explicit.
- Mermaid or ZenUML baseline movement requires a deliberate package version and a rerun of the
  one-way fixture oracle rather than automatic coupling to the workspace release.

## Rejected Alternatives

### Replace Merman's semantic parsers with Tree-sitter

Rejected. Concrete syntax and incremental recovery do not replace semantic construction, parser
side effects, typed models, diagnostics, or refactoring identity. There is no equivalence evidence.

### Feed Tree-sitter CST semantic facts into editor-core or LSP

Rejected. That would recreate the second successful editor grammar removed by ADR-0071 and permit
semantic disagreement with rendering. ADR-0084 separately permits syntax-only highlight captures
inside the LSP adapter.

### Keep the language package in a separate repository

Rejected for initial development. Atomic review with the pinned Mermaid source, family catalog,
fixtures, legal inventory, and one-way oracle is more valuable until the conformance contract is
stable.

### Claim coverage through a generic line fallback

Rejected. Recognition without useful named structure does not provide the editor value that
justifies maintaining the second syntax implementation.

### Maintain a proof platform around the grammar

Rejected. Receipts, support tiers, schema mirrors, metrics evidence, editor applicability matrices,
and duplicate capture goldens increase maintenance cost without improving the parser product. Use
the existing Tree-sitter, Rust, Node, Cargo, npm, CMake, and fuzz tools directly.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0071: Editor-Facing Parser and Semantic Seam
- ADR-0073: Family-Owned Diagram Architecture
- ADR-0075: ZenUML Parser Technology
- ADR-0076: Capability-Driven Feature and Package Surfaces
- ADR-0081: Release Quality Gates
- ADR-0084: Tree-sitter Highlighting Ownership

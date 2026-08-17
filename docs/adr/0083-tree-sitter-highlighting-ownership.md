# ADR-0083: Tree-sitter Highlighting Ownership

## Status

Accepted

## Dates

- Accepted: 2026-08-17

## Context

Merman historically produced editor coloring from parser-owned `EditorLexeme*` facts. Family
parsers emitted lexical journals, `merman-editor-core` combined lexical and semantic facts in one
token planner, and the LSP and Playground transported the resulting packed tokens.

The repository now also maintains `distribution/tree-sitter-mermaid`, a tolerant, incremental CST
and query product covering every public Mermaid family. Keeping both implementations means that a
syntax change can require parallel edits to family lexeme emission and the Tree-sitter highlight
query even when strict parsing, semantic construction, and rendering have not changed.

Tree-sitter is a better fit for syntax coloring of incomplete buffers, but it does not reproduce
Mermaid's stateful semantic construction, validation, database mutation order, identity, safe
refactoring, IR, or render behavior. The LSP protocol name `semanticTokens` is a transport name; it
does not require the token producer to own semantic identity.

## Decision

### Tree-sitter exclusively owns syntax highlighting

`distribution/tree-sitter-mermaid/grammar.js` and
`queries/portable/highlights.scm` are the single implementation of base syntax highlighting for
Merman's native LSP and Playground surfaces.

- The native LSP uses the Rust `tree-sitter-mermaid` grammar crate and the pinned Tree-sitter
  runtime.
- The Playground uses `web-tree-sitter`, the package language WASM, and the same canonical query in
  a dedicated browser worker.
- Runtime adapters may map portable captures to standard LSP or Monaco token types, split
  multiline captures, convert coordinates to UTF-16, and resolve overlaps. They must not infer
  Mermaid syntax through regular expressions or Merman semantic facts.
- A Tree-sitter failure is explicit. Neither adapter falls back to a hidden Merman, Monarch, or
  regular-expression highlighter.

The native and browser runtimes are platform adapters over one grammar and query contract, not two
independent Mermaid syntax implementations.

### Merman exclusively owns strict semantics

Merman's strict parser and analysis stack remains the sole authority for:

- accepted syntax and strict validity;
- semantic construction and family database mutation order;
- diagnostics and expected syntax;
- completion, hover, document symbols, definitions, references, and safe rename;
- semantic identity, source mapping, IR, layout inputs, and rendering.

Tree-sitter recovery does not establish Mermaid validity. `locals.scm` and `tags.scm` remain
best-effort ecosystem queries and must not become identity, navigation, or rename evidence.

### Syntax state is independent from semantic state

Syntax highlighting must remain available while strict analysis is delayed, unavailable, or has
rejected an incomplete buffer.

- An open Mermaid document owns an incremental syntax tree that is independent of
  `AnalysisGeneration` and semantic-worker readiness.
- Markdown and MDX hosts use a lightweight syntax-side fence segmenter and one Mermaid tree per
  discovered fence. Tree-sitter parses fence bodies, not host backticks.
- Invalid incremental edits may fall back to a fresh syntax parse. Source-limit or synchronization
  loss invalidates syntax state until a valid full replacement arrives rather than reusing stale
  semantic state.
- Tolerant syntax captures and strict semantic results retain separate version and failure
  lifecycles.

### Production dependency exceptions are narrow

The Rust Tree-sitter runtime and grammar are allowed only in the exact `lsp-library` and
`lsp-stdio-release` artifact profiles. They remain forbidden from the production dependency
closures of core, analysis, editor-core, render, IR, Web and WebAssembly, CLI, and
language-binding artifacts.

The Playground exception is a browser asset boundary, not a Rust dependency exception. It loads
the external `web-tree-sitter` runtime and stages the canonical grammar WASM and query; it does not
compile Tree-sitter into Merman's existing `wasm32-unknown-unknown` artifact.

### The replaced lexical implementation is retired

The coordinated cutover removes `EditorLexeme*`, family-local lexeme emission, the mixed token
planner, packed-token equivalence evidence, and obsolete Merman WASM/browser token exports. It also
moves the standard token legends into the LSP and Playground adapters.

This deletion does not remove parser-owned semantic symbols, expected syntax, rename policy,
source maps, recovery diagnostics, or the protocol-neutral editor features derived from them.

### Verification stays conventional

The ownership boundary is protected by standard Tree-sitter corpus/query tests, compact
all-family capture expectations, focused native/browser projection tests, existing LSP protocol
tests, one browser smoke, and artifact dependency-closure checks. The repository does not add a
second proof engine, per-editor capture matrix, receipt graph, or permanent old-versus-new token
harness.

## Amendments to Earlier Decisions

This ADR supersedes only the highlighting clauses of ADR-0071 and ADR-0082:

- ADR-0071's family-owned lexical journal and `merman-editor-core` token-planner ownership are
  retired. Its parser-owned semantic identity, expected-syntax, diagnostics, source-mapping, and
  refactoring-safety decisions remain in force.
- ADR-0082's blanket prohibition on production Merman dependencies is narrowed for the two LSP
  artifact profiles, and its rejection of LSP CST use is narrowed to semantic use. Its strict
  parser boundary, one-way conformance strategy, lean verification policy, and independent
  distribution decision remain in force.

This ADR does not authorize replacing LALRPOP, Jison-compatible, Langium-compatible, or handwritten
family parsers with Tree-sitter.

## Consequences

- Mermaid syntax-coloring maintenance has one grammar/query owner shared by internal and external
  editor consumers.
- The LSP and Playground retain separate runtime and coordinate-projection code, but neither owns
  another Mermaid grammar.
- Incomplete text can remain colored without treating it as strictly valid or waiting for Merman
  analysis.
- LSP artifacts gain Tree-sitter runtime and generated-parser size; other Merman Rust artifacts do
  not.
- Removing the old lexical stack is an intentional internal and prerelease API break.
- A Mermaid baseline upgrade may still require both strict-parser and Tree-sitter grammar changes
  when accepted syntax changes. Query-only coloring changes remain localized to the Tree-sitter
  package.

## Rejected Alternatives

### Replace Merman's strict parser portfolio with Tree-sitter

Rejected. A tolerant CST does not implement semantic construction, validation side effects,
identity, refactoring safety, IR, or render behavior.

### Keep Merman lexemes as an overlay or fallback

Rejected. A permanent overlay preserves the duplicate syntax implementation and makes precedence,
failure, and maintenance ownership ambiguous.

### Use Tree-sitter locals or tags for navigation and rename

Rejected. Portable ecosystem queries are useful discovery aids, not proof of family-owned semantic
identity or safe edit constraints.

### Compile Tree-sitter into the existing Merman browser WASM

Rejected. The generated C parser expects a libc-backed target. The standard `web-tree-sitter`
runtime and language WASM provide the browser integration without adding a custom C/WASM toolchain
to `merman-wasm`.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0071: Editor-Facing Parser and Semantic Seam
- ADR-0073: Family-Owned Diagram Architecture
- ADR-0074: Browser Runtime and Benchmark Ownership
- ADR-0076: Capability-Driven Feature and Package Surfaces
- ADR-0081: Release Quality Gates
- ADR-0082: Tree-sitter Mermaid Language Boundary

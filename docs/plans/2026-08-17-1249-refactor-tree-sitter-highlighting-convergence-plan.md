---
title: "Tree-sitter Highlighting Convergence Plan"
type: "refactor"
date: "2026-08-17"
artifact_contract: "ce-unified-plan/v1"
artifact_readiness: "implementation-ready"
product_contract_source: "maintainer-direction"
execution: "code"
origin_plans:
  - "docs/plans/2026-08-14-001-feat-tree-sitter-mermaid-language-plan.md"
  - "docs/plans/2026-08-16-001-refactor-tree-sitter-mermaid-distribution-plan.md"
---

# Tree-sitter Highlighting Convergence Plan

## Goal Capsule

- Make the canonical Tree-sitter Mermaid grammar and `queries/portable/highlights.scm` the only
  implementation of syntax highlighting in Merman's Playground and native LSP surfaces.
- Keep `merman-core` and `merman-analysis` as the only owners of strict validity, semantic models,
  diagnostics, completion candidates, identity resolution, navigation, safe rename, IR, and
  rendering.
- Delete the replaced `EditorLexeme*` journal, family-local lexeme emission, mixed token planner,
  generated token-equivalence evidence, and browser/WASM token route instead of preserving a
  permanent shadow implementation.
- Expose the existing publishable `tree-sitter-mermaid` Rust/npm/WASM distribution directly to
  consumers. Native LSP uses the Rust Tree-sitter runtime; the Playground uses `web-tree-sitter`
  and the shipped language WASM. Both execute the same generated grammar and canonical query.
- Keep verification conventional and small: standard Tree-sitter corpus/query tests, focused
  projection tests, existing LSP tests, and one browser smoke. Do not introduce a new evidence
  engine, capture receipt graph, or combinatorial per-runtime or per-editor matrix.

This plan supersedes only the following boundaries in the earlier Tree-sitter plans, ADR-0071, and
ADR-0082:

- production adapters may now depend on Tree-sitter for syntax highlighting;
- the Playground no longer keeps Merman semantic tokens as its default coloring path; and
- the LSP may consume the tolerant CST and canonical highlight query for token transport; and
- ADR-0071's parser-owned lexical-highlighting projection is retired while its parser-owned
  semantic identity, expected-syntax, diagnostic, and refactoring-safety boundaries remain.

It does not supersede the strict semantic-parser boundary, the one-way conformance strategy, the
lean distribution strategy, or the prohibition on using Tree-sitter CST as render/IR authority.

---

## Product Contract

### Problem Frame

Merman currently maintains syntax-coloring knowledge twice. The strict parser portfolio emits
`EditorLexeme*` facts from dozens of family modules, and `merman-editor-core` combines those facts
with semantic symbols in a large token planner. The Tree-sitter package separately owns a
source-backed, tolerant grammar for all public Mermaid families and a canonical highlights query.

The duplication has three concrete costs:

- Mermaid syntax changes can require matching edits in a family parser's lexeme journal and in a
  Tree-sitter query even when strict parsing and rendering semantics did not change.
- The Playground performs a full Merman document analysis on each content change even though only
  diagnostics are debounced, coupling immediate coloring to the heavier semantic pipeline.
- External editors, Merman's LSP, and the Playground do not benefit from the same highlighting
  implementation even though the distribution already ships Rust, Node, C, and WASM surfaces.

Tree-sitter is suitable for incomplete-buffer CST and syntax coloring, but it is not a replacement
for Mermaid's stateful semantic construction. Treating locals/tags queries as a rename or identity
engine would recreate the second semantic grammar that ADR-0071 removed.

### Key Decisions

- **KTD1 — Tree-sitter exclusively owns base syntax highlighting.** The canonical highlight query
  is the only source of token kind and span for Playground and LSP coloring. Merman does not
  overlay, recolor, or create base tokens. `[session-settled: user-approved]`
- **KTD2 — Merman exclusively owns strict semantics.** Completion values, hover details,
  definitions, references, rename validation, diagnostics, IR, and render behavior continue to
  use parser-owned semantic facts. Tree-sitter recovery does not imply strict validity.
  `[session-settled: user-approved]`
- **KTD3 — One language implementation, two runtime adapters.** Native LSP links the Rust grammar
  crate and Tree-sitter runtime. The browser loads `web-tree-sitter` and the grammar WASM in a
  dedicated syntax worker. Both use the same grammar and query; runtime projection code remains a
  thin platform adapter. `[session-settled: user-approved]`
- **KTD4 — Do not compile Tree-sitter into Merman's current browser WASM.** The generated C parser
  requires a libc-backed target and does not build for the repository's current
  `wasm32-unknown-unknown` Merman artifact. The browser therefore uses the standard
  `web-tree-sitter` runtime rather than adding a custom C/WASM toolchain to `merman-wasm`.
  `[session-settled: evidence-backed]`
- **KTD5 — Replace rather than layer.** Temporary differential checks may guide the migration, but
  the merged result deletes the old lexeme/token implementation and its equivalence fixtures.
  Internal API and wire breaks are allowed where the replaced token surface is not an external
  compatibility promise. `[session-settled: user-directed]`
- **KTD6 — Keep verification lean.** Use existing Tree-sitter, Rust, Node, TypeScript, and browser
  test runners. Add only tests that protect the new ownership boundary or a non-trivial projection
  invariant; do not add another proof framework. `[session-settled: user-directed]`

### Requirements

#### Syntax ownership

- R1. `distribution/tree-sitter-mermaid/queries/portable/highlights.scm` is the canonical syntax
  coloring contract for Merman-owned editor surfaces and external consumers.
- R2. The query uses portable Tree-sitter capture names. Platform adapters may collapse a capture
  into a standard LSP/Monaco token type, but may not infer Mermaid syntax with regexes or semantic
  model inspection.
- R3. Every public Mermaid family has a compact expected set of user-visible body capture classes,
  not merely a diagram-header keyword. The set lives in the existing query test table rather than
  a new capture-forest format, and intentionally uncolored syntax is explicit. A small exact-span
  set covers the capture classes that are easy to get wrong: identifiers, operators, punctuation,
  strings, comments, dates/durations, and styles.
- R4. Incomplete or recovered input still yields local captures for the surrounding valid syntax;
  a malformed statement must not erase later sibling highlighting.
- R5. Frontmatter and directives receive explicit syntax treatment. YAML content may be exposed
  through a standard injection if it remains portable; otherwise the query must at least classify
  the bounded frontmatter region without inventing a YAML parser.

#### Native LSP

- R6. `merman-lsp` may depend directly on the local `tree-sitter-mermaid` crate and pinned
  Tree-sitter runtime. `merman-core`, `merman-analysis`, render crates, and the IR pipeline must not.
  Artifact dependency-closure policy grants the exception only to the LSP library and stdio
  release profiles; Web WASM, CLI, bindings, and render profiles keep rejecting Tree-sitter.
- R7. LSP full, range, and delta semantic-token protocol behavior, client capability negotiation,
  result-id invalidation, cancellation, and stale-write suppression remain owned by `merman-lsp`.
  Only the token producer changes.
- R8. The native highlighter keeps a document-owned `SyntaxDocumentState` that is independent of
  `AnalysisGeneration`, strict-analysis snapshots, and semantic-worker readiness. A Mermaid
  document owns one incremental Tree-sitter tree; a Markdown/MDX host owns one tree per discovered
  Mermaid fence. Text edits update syntax state before token requests, and a fresh parse remains
  the correctness fallback after an invalid incremental edit or parser failure. Source-limit or
  synchronization-loss failures invalidate syntax state until the next valid full replacement
  rather than borrowing a stale semantic snapshot.
- R9. Capture projection produces sorted, non-overlapping, multiline-split UTF-16 tokens and
  handles CRLF and astral Unicode without depending on Merman semantic facts.
- R9a. Markdown and MDX host structure, Mermaid-fence discovery, and host/source coordinate mapping
  are maintained by a lightweight syntax-side host segmenter, without requiring a completed Merman
  analysis. Tree-sitter parses each Mermaid fence body, not the host document or its backtick
  delimiters. Strict semantics may keep its own document snapshot, but syntax-token availability
  must not depend on that snapshot's generation or success.

#### Playground

- R10. The Playground exposes one language-module seam to Monaco while internally using a syntax
  worker and the existing Merman semantic worker. React/editor callers do not manage runtimes,
  language WASM URLs, token legends, document versions, or coordinate conversion.
- R11. Content updates reach the syntax worker immediately. Merman semantic analysis is debounced
  for diagnostics and flushed on demand before completion, hover, navigation, or rename requests.
  No stale semantic revision may be presented as current.
- R12. Syntax-worker failure is isolated from semantic features, and semantic-worker failure is
  isolated from syntax highlighting. Either failure is reported through the existing status/error
  surface rather than silently switching to a second highlighter.
- R13. The browser loads `web-tree-sitter`, the npm-shipped language WASM, and the canonical query
  as lazy editor assets. Their loading must be compatible with the Playground's Vite build, worker
  bundling, public base path, and content-security policy.
- R13a. Inside the monorepo, the Playground depends directly on `web-tree-sitter` and stages the
  grammar WASM/query from `distribution/tree-sitter-mermaid` during its existing asset build. It
  does not install the grammar package root and trigger the native Node binding's install hook.
  External browser consumers continue to obtain the same assets from the published npm package.
- R14. Monaco may continue using its document-semantic-token transport API, but the producer and
  implementation are named as syntax highlighting internally; the transport name does not grant
  Merman ownership of coloring.
- R14a. Token-legend and token-descriptor ownership moves to the two Tree-sitter adapters before the
  old descriptor is removed. The Playground Merman-worker protocol drops `semanticTokens` and
  `legendDigest` in an explicit protocol-version bump. The LSP keeps the standard
  `semanticTokensProvider` legend required by the protocol, while Merman-specific experimental
  capability digests and VS Code descriptor validation are removed. The adapters use a small,
  checked-in standard capture-to-token map; they do not regenerate a Merman semantic-token
  descriptor under a new name.

#### Semantic boundary and deletion

- R15. Keep parser-owned `EditorSemanticSymbol`, `EditorExpectedSyntax`, rename policy, source-map,
  recovery-diagnostic, completion, hover, document-symbol, definition, reference, and rename facts
  until a separate product decision explicitly replaces them.
- R16. Delete `EditorLexemeKind`, modifiers, producer/failure/batch/journal types, lexeme fields on
  semantic facts, family-local lexeme emission, lexeme remapping/finalization, and analysis-side
  lexeme indexes after both editor surfaces use Tree-sitter.
- R17. Replace the mixed `token_planner` with the smallest protocol-specific projection needed by
  LSP. Remove its semantic overlay precedence, family token matrices, packed-token equivalence
  evidence, and Merman WASM/web semantic-token exports.
- R17a. Split rename-policy generation/ownership away from the combined token descriptor before
  deleting that descriptor. The cutover must not remove family-owned safe-rename constraints merely
  because their current generator shares a file with token metadata.
- R18. Update VS Code, web package, capability documentation, and public Rust documentation to stop
  promising Merman-parser-produced coloring. Standard LSP semantic-token capability remains.
- R19. Tree-sitter `locals.scm` and `tags.scm` remain best-effort ecosystem queries and are not used
  as proof of Merman identity, rename, or navigation correctness.

#### Distribution and maintenance

- R20. The existing `tree-sitter-mermaid` crate/npm package remains the only grammar distribution;
  do not add a Playground-only grammar copy or a second TypeScript parser package.
- R21. Query and grammar baseline changes use the existing one-way strict-valid fixture oracle and
  standard corpus tests. Do not restore support-tier metadata, receipts, per-editor applicability
  matrices, or exact 35-family packed-word snapshots.
- R22. A Mermaid baseline upgrade still changes strict parsers and Tree-sitter grammar when syntax
  changes, but common fixtures and the one-way oracle expose divergence. Query-only coloring
  changes remain localized to the Tree-sitter package.
- R23. Registry publication follows dependency order: publish and verify the exact
  `tree-sitter-mermaid` crate version before publishing a `merman-lsp` crate that depends on it, and
  publish the npm grammar package before documenting it as an external browser dependency.
  Registry writes require separate maintainer authorization; this plan may prepare and dry-run
  packages but does not infer permission to publish them.

### Scope Boundaries

In scope:

- canonical highlight-query hardening;
- a native Rust Tree-sitter highlighter inside the LSP adapter;
- a `web-tree-sitter` syntax worker and Playground coordinator;
- delayed/on-demand Merman semantic synchronization;
- removal of the old Merman lexical-token stack and its generated/browser contracts;
- documentation and release-package adjustments required by those changes.

Out of scope:

- replacing LALRPOP, Langium-compatible, Jison-compatible, or handwritten strict parsers;
- deriving Merman IR, render behavior, diagnostics, or rename identity from a Tree-sitter CST;
- migrating completion/navigation/rename to `locals.scm` or `tags.scm`;
- adding Python, Go, Swift, or a custom browser parsing SDK;
- proving every capture in every downstream editor or maintaining a permanent old/new differential
  harness;
- redesigning Monaco themes or the Playground user interface beyond runtime/status wiring.

### Acceptance Examples

- AE1. Editing `flowchart TD\nA --> B` to an incomplete edge updates visible syntax colors without
  waiting for Merman analysis, while the next diagnostic pass still comes from Merman.
- AE2. An emoji identifier and a multiline quoted value produce valid, sorted UTF-16 token ranges
  in both the native LSP projection and browser projection.
- AE3. An incremental syntax update and a fresh Tree-sitter parse produce the same projected token
  sequence for representative ordinary, multiline, and indentation-sensitive edits.
- AE4. A blocked or failed Merman semantic worker does not remove Tree-sitter highlighting. A
  failed Tree-sitter worker does not make completion/diagnostics pretend to be current.
- AE5. LSP full/range/delta requests preserve existing result semantics while their token spans and
  kinds come only from the canonical Tree-sitter query.
- AE6. `rg 'EditorLexeme|replace_family_lexemes|lexeme_failure' crates` finds no production lexical
  highlighting path after migration; remaining matches, if any, are explicit historical docs.
- AE7. The Playground production build contains the shipped grammar WASM and `web-tree-sitter`
  runtime once, starts successfully under the configured base path, and highlights an incomplete
  Mermaid document.
- AE8. Existing completion, hover, definition, references, rename, diagnostics, IR, and rendering
  tests continue to use Merman semantic facts and pass without Tree-sitter dependencies in their
  production crates.

---

## Planning Contract

### Assumptions

- The current `tree-sitter-mermaid` package remains pre-1.0, so capture and generated-node fixes may
  be made in the same branch with clear release notes.
- Existing external users of Merman's direct WASM/Rust packed semantic-token helper may experience
  a breaking removal. The repository is authorized to break and delete this unreleased duplicate
  surface rather than maintain a compatibility adapter.
- A small static mapping from portable Tree-sitter capture names to standard LSP/Monaco token types
  belongs in each runtime adapter or in a tiny package-owned data file. It must not become another
  generated descriptor system.
- The first browser implementation may continue sending a full source snapshot to the syntax
  worker and derive a bounded single edit internally. Exact Monaco change batches should be added
  only if measurement shows the full snapshot to be the dominant cost.
- Merman semantic synchronization keeps the existing document-version contract. The coordinator
  may retain the latest source locally and flush it before a semantic query rather than adding a
  second public editor protocol.

### High-Level Technical Design

```text
                           canonical language product
                  grammar + generated parser + highlights.scm
                                    |
                   +----------------+----------------+
                   |                                 |
             Native Rust adapter               Browser adapter
             tree-sitter runtime               web-tree-sitter
                   |                                 |
        merman-lsp semantic-token path       Playground syntax worker
                   |                                 |
                   +----------- editor UI -----------+
                                    |
                     Merman semantic worker / core
        diagnostics, completion, hover, symbols, navigation, rename
                                    |
                        strict models, IR, rendering
```

The shared implementation is the grammar and query. Native and browser adapters own only runtime
lifecycle, document edits, coordinate projection, token packing, and platform error mapping.

### Sequencing

1. Record the superseding architecture decision and freeze the canonical capture contract.
2. Harden query coverage and package-level capture invariants before changing either production
   surface.
3. Implement native LSP and browser adapters independently against that contract.
4. Cut both surfaces over, then delete the old lexical implementation in one coordinated break.
5. Remove obsolete contracts/tests/docs, run focused verification, simplify, review, and commit.

U3 and U4 may proceed independently after U2. U5 must wait until both production surfaces no longer
consume Merman lexemes.

### Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Query captures look correct in headers but miss body syntax | Premature cutover produces visibly weak coloring | Require an expected body capture-class set for every public family and exact spans for a small set of capture archetypes |
| UTF-8 byte spans are projected incorrectly to UTF-16 | Monaco/LSP tokens shift around emoji or CRLF | Centralize conversion per adapter and cover astral, CRLF, multiline, and overlap cases |
| Browser ships two heavy parsers on every keystroke | Typing latency regresses | Immediate incremental syntax worker; debounce/lazy Merman semantic synchronization; lazy parallel asset loading |
| Adapter mapping drifts between Rust and TypeScript | Playground and LSP color differently | Use the same portable capture vocabulary and one compact runtime-neutral fixture table; avoid language-specific inference in either adapter. This is not a combinatorial per-runtime or per-editor matrix. |
| Deleting lexical facts accidentally removes semantic identity | Completion/navigation/rename regress | Delete only lexeme-owned fields and tests; retain semantic-symbol, expected-syntax, rename-policy, and diagnostic paths |
| Tree-sitter is mistaken for strict validity | Rendering or diagnostics diverge from Mermaid | Keep dependency out of core/analysis/render, document the recovery boundary, retain one-way strict-valid conformance |
| Existing direct token API consumers break silently | Downstream build or behavior regression | Remove/export-break explicitly, update changelog/docs, and keep LSP capability as the supported token surface |

---

## Implementation Units

### U1. Supersede the old highlighting boundary

- **Goal:** Make syntax-versus-semantics ownership explicit before changing code.
- **Files:**
  - new `docs/adr/0083-tree-sitter-highlighting-ownership.md`;
  - `docs/adr/0082-tree-sitter-language-boundary.md` related-decision note;
  - `docs/adr/0071-editor-parser-semantic-seam.md` related-decision note;
  - `docs/development/TREE_SITTER_MERMAID.md`;
  - `scripts/verify_artifact_dependency_closures.py`;
  - relevant package/editor README wording.
- **Approach:** Add a new ADR rather than rewriting history. It should amend ADR-0082's rejection of
  production LSP use and ADR-0071's lexical-fact ownership, while explicitly preserving their
  strict semantic and parser-owned identity decisions.
- **Test scenarios:** Documentation links resolve; dependency-boundary wording matches the code
  units below; no document claims Tree-sitter owns strict validity or rename semantics.
- **Done when:** The approved architecture and intentional breaks are reviewable without inferring
  them from implementation details.

### U2. Harden the canonical highlight contract

- **Goal:** Make the existing query good enough to be the sole highlighter without creating a new
  evidence system.
- **Files:**
  - `distribution/tree-sitter-mermaid/queries/portable/highlights.scm`;
  - `distribution/tree-sitter-mermaid/tests/queries.rs`;
  - only family grammar/corpus files needed to fix a proven capture boundary;
  - package README/query documentation.
- **Approach:** Extend the current representative-family table so every family proves a compact
  expected body capture-class set rather than a single capture. Add a compact exact-span table for
  capture archetypes, plus Unicode/CRLF/multiline, incomplete sibling recovery, and
  incremental-equals-fresh query cases. Prefer query fixes; change grammar nodes only where token
  boundaries include structural whitespace or hide required syntax.
- **Test scenarios:** query compilation; 35 family expected body capture-class sets; exact
  identifier/operator/punctuation/string/comment/date/duration/style spans; frontmatter/directive
  behavior; incremental/fresh capture equivalence; captures remain inside source and node bounds.
- **Done when:** The package exposes an adapter-ready canonical capture contract without Merman
  lexical facts or family-specific post-processing. Projected token ordering, overlap, and UTF-16
  behavior remain U3/U4 adapter responsibilities.

### U3. Move native LSP highlighting to Tree-sitter

- **Goal:** Preserve LSP semantic-token protocol behavior while replacing its token source.
- **Files:**
  - `crates/merman-lsp/Cargo.toml`;
  - `crates/merman-lsp/src/semantic_tokens.rs`;
  - a focused internal syntax-document/highlight module if needed;
  - LSP document store/session code that owns edit/version lifecycle;
  - artifact dependency-closure expectations for LSP-only use;
  - LSP token tests and capabilities documentation.
- **Approach:** Store an incremental Tree-sitter tree with the open document state, apply edits in
  document order, execute the canonical query for full/range requests, project captures to standard
  LSP token types, and feed the existing full/delta result machinery. Keep cancellation and stale
  document-version guards at their current adapter boundary. The syntax state is a separate,
  document-owned lifecycle and never waits for `AnalysisGeneration`. For Markdown/MDX, update a
  lightweight host fence/source map directly from document text and run one Mermaid parser/query
  per fence body. Invalid incremental state falls back to fresh syntax parsing; source-limit or
  synchronization loss remains invalid until a valid full-document replacement arrives.
- **Test scenarios:** full/range/delta behavior; stale result IDs; capability negotiation; CRLF,
  astral Unicode, multiline splitting, overlap resolution; incremental/fresh equality; parser
  cancellation/fallback; no dependency from core/analysis/render crates.
- **Done when:** LSP token output has no call path into `merman-editor-core`'s old token planner or
  `EditorLexeme` storage.

### U4. Add the Playground Tree-sitter syntax worker

- **Goal:** Provide immediate tolerant highlighting without running full Merman analysis on every
  keystroke.
- **Files:**
  - `playground/package.json` and lockfile;
  - Playground Vite/worker asset configuration as needed;
  - new syntax worker, syntax document, and token projector modules;
  - `playground/src/lib/mermaid-language.ts` coordinator;
  - existing editor worker protocol/runtime code for delayed semantic synchronization;
  - focused unit and browser smoke tests.
- **Approach:** Load `web-tree-sitter`, the package WASM, and the canonical query in a dedicated
  worker. Keep one Monaco-facing module, send source revisions immediately to syntax, retain the
  latest revision for Merman, debounce diagnostics, and flush semantics before semantic requests.
  Stage the canonical grammar/query assets through the existing Playground build rather than
  installing the native grammar package root. Report worker failures independently; do not fall
  back to the deleted Merman highlighter. Bump the Merman-worker protocol and remove its token
  payload/digest fields; the Monaco language module owns the standard token legend and consumes
  syntax-worker results directly.
- **Test scenarios:** worker initialization and disposal; stale-version rejection; incremental/
  fresh token equality; sorted non-overlapping UTF-16 output; emoji/CRLF/multiline; semantic worker
  delay/failure does not block syntax; production build resolves WASM/query assets under the public
  base path.
- **Done when:** The Playground's Monaco token provider consumes only syntax-worker results, and a
  content change no longer immediately rebuilds Merman analysis unless a semantic request demands
  it.

### U5. Delete the replaced Merman lexical stack

- **Goal:** Finish the convergence by removing the second syntax-coloring implementation.
- **Files:**
  - `crates/merman-core/src/editor.rs` and family/preprocess lexeme call sites;
  - `crates/merman-analysis/src/editor.rs` and lexeme storage/accessors;
  - `crates/merman-editor-core/src/token_planner.rs`, exports, README, and token-planner tests;
  - `crates/merman-wasm/src/editor_language.rs` and web bindings;
  - `contracts/editor-language/` token descriptor/equivalence artifacts;
  - `crates/xtask/src/cmd/editor_token_descriptor.rs` and rename-policy generation ownership;
  - VS Code/web generated token descriptors and smokes;
  - lexeme-specific core tests.
- **Approach:** Remove types and producers from the leaves inward: family emission, core journal,
  analysis storage, planner, WASM/web exports, generated descriptors, and tests. Preserve semantic
  symbols, expected syntax, rename policy, source mapping, recovery diagnostics, and their public
  APIs. First split rename-policy ownership from the combined token descriptor/generator. Then move
  standard token legends into the LSP and Playground adapters, remove Merman-worker
  `semanticTokens`/`legendDigest` fields with a protocol-version bump, and remove the LSP
  experimental descriptor digest plus VS Code validation while retaining the standard LSP
  capability. Only after those consumers migrate may token metadata be deleted. Do not regenerate
  old evidence in a new shape.
- **Test scenarios:** core semantic facts still drive completion/navigation/rename/diagnostics;
  public crates compile without lexical types; no production lexeme matches remain; LSP and
  Playground highlighters continue to pass after the deletion.
- **Done when:** Removing the Tree-sitter adapters would leave no hidden Merman syntax highlighter,
  while removing Tree-sitter has no effect on strict parse/render semantics.

### U6. Finish distribution, documentation, and release-facing cleanup

- **Goal:** Make the new ownership understandable and shippable without adding bespoke tooling.
- **Files:**
  - `distribution/tree-sitter-mermaid/package.json`, exports, README, and package smoke only if the
    Playground exposes a missing standard asset;
  - workspace/package dependency metadata and generated legal files as required by existing tools;
  - Playground, LSP, editor-core, web, and VS Code documentation;
  - changelog/release notes appropriate to the breaking token API removal.
- **Approach:** Confirm the npm package is the browser asset source and the Rust crate is the native
  grammar source. Remove stale claims about Merman-produced coloring and about Tree-sitter being
  unused by production adapters. Keep release validation in existing Cargo/npm/CI paths. Prepare
  registry dry-runs in dependency order, but do not publish without separate maintainer
  authorization: the exact `tree-sitter-mermaid` crate version must exist before a dependent
  `merman-lsp` release, and the npm grammar package must exist before external browser-consumer
  instructions rely on it.
- **Test scenarios:** crate/package contents; Playground production build; LSP and package tests;
  documentation link checks where already available; no restored receipt or per-editor matrix.
- **Done when:** Local package dry-runs prove the dependency order, users can consume
  grammar/query/WASM once the separately authorized registry publication occurs, the Playground
  visibly uses the staged canonical assets, and repository documentation presents one syntax
  highlighter plus one strict semantic engine.

---

## Verification Contract

Run the smallest command that proves each unit, then a final integrated pass. Avoid parallel Cargo
jobs when other repository work is using the shared target directory.

| Verification | Units | Purpose |
| --- | --- | --- |
| package-local `tree-sitter test` | U2 | CST and recovery expectations remain valid |
| `cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast` | U2, U6 | Query, conformance, incremental, and binding behavior |
| `cargo nextest run --locked -p merman-lsp --no-fail-fast` | U3, U5 | Native token protocol and semantic features |
| focused `merman-core`, `merman-analysis`, and `merman-editor-core` nextest suites | U5 | Deletion preserves non-highlighting semantics |
| Playground unit tests and production build | U4, U6 | Worker protocol, projection, asset bundling |
| one browser smoke on an incomplete source | U4, U6 | Real Monaco/worker/WebAssembly behavior |
| `python3 scripts/verify_artifact_dependency_closures.py` | U1, U3, U6 | Tree-sitter stays limited to LSP profiles |
| `cargo fmt --all -- --check` | U1-U6 | Rust formatting |
| scoped Clippy for changed Rust packages | U2, U3, U5 | Warnings and API cleanup |
| `rg` deletion checks for old lexical symbols/contracts | U5, U6 | No shadow implementation remains |

Permanent verification is intentionally limited to:

1. standard Tree-sitter corpus and query compilation;
2. one representative 35-family expected capture-class table plus compact exact-span archetypes;
3. incremental/fresh projection equality;
4. native LSP token protocol tests;
5. browser worker/projector tests and one real browser smoke; and
6. existing strict semantic and render suites.

Any migration-only old-versus-new token comparison must be deleted before final review.

---

## Definition of Done

- Tree-sitter is the sole syntax-token producer in Playground and LSP.
- The Playground runs syntax parsing in its own worker and no longer performs immediate full Merman
  analysis solely to color each edit.
- LSP full/range/delta token requests use the canonical Tree-sitter query and retain their protocol
  guarantees.
- `EditorLexeme*`, family lexeme emission, the mixed token planner, obsolete WASM/web token exports,
  and token-equivalence evidence are deleted.
- Merman strict parsing, diagnostics, completion, hover, symbols, definition, references, rename,
  IR, and rendering remain parser-owned and pass their focused suites.
- The canonical query has body evidence for all public families and focused Unicode, CRLF,
  multiline, incomplete, and incremental coverage.
- The existing Rust/npm/WASM Tree-sitter distribution is the only grammar package used by internal
  and external consumers.
- No new proof engine, receipt system, per-editor matrix, or permanent dual-highlighter comparison
  is introduced.
- Documentation and a superseding ADR describe the final ownership boundary and breaking changes.
- The implementation is simplified, independently reviewed, formatted, tested, and committed in
  coherent Conventional Commit units.

---

## Sources

- `docs/adr/0071-editor-parser-semantic-seam.md`
- `docs/adr/0082-tree-sitter-language-boundary.md`
- `docs/plans/2026-08-14-001-feat-tree-sitter-mermaid-language-plan.md`
- `docs/plans/2026-08-16-001-refactor-tree-sitter-mermaid-distribution-plan.md`
- `distribution/tree-sitter-mermaid/grammar.js`
- `distribution/tree-sitter-mermaid/queries/portable/highlights.scm`
- `distribution/tree-sitter-mermaid/tests/queries.rs`
- `crates/merman-core/src/editor.rs`
- `crates/merman-analysis/src/editor.rs`
- `crates/merman-editor-core/src/token_planner.rs`
- `crates/merman-lsp/src/semantic_tokens.rs`
- `crates/merman-wasm/src/editor_language.rs`
- `playground/src/lib/mermaid-language.ts`
- `playground/src/editor/worker-runtime.ts`

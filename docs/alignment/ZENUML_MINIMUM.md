# ZenUML Family Contract

Merman implements ZenUML as a family-owned grammar and semantic model. It does not translate
ZenUML into Mermaid Sequence actions or Sequence JSON.

## Authority

- Mermaid baseline: `mermaid@11.16.1`.
- Selected behavior source: `@zenuml/core@3.50.1`, commit
  `38404ccc14243ed54ab45b804b2eb6f2ca73af36`.
- Selected grammar: `repo-ref/zenuml-core/src/g4/sequenceLexer.g4` and
  `repo-ref/zenuml-core/src/g4/sequenceParser.g4`.
- The historical `3.47.8` to `3.50.1` decision is retained by
  `tools/upstreams/MERMAID_SELECTION_DECISION.json`; it is not a second live package graph.
- New compatible or outside-range releases are evaluated only by the manual Mermaid upgrade
  admission workflow and do not appear in the standing reference bundle before selection.

The current graph is `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`. Its decision-receipt path and
digest bind `tools/upstreams/MERMAID_SELECTION_DECISION.json`.

ADR-0075 records the parser technology decision. The selected grammar's lexer channels/modes,
runtime semantic predicates, Unicode lookahead, incomplete-input recovery, and exact source spans
are implemented by a grammar-derived Unicode token scanner plus bounded recursive descent. Parser
generator uniformity is not a capability claim or architecture invariant.

## Owned pipeline

`crates/merman-core/src/diagrams/zenuml/` contains the lexer, recursive recovering parser, AST,
semantic builder, editor projection, and typed render model. The family registration points every
entry point at one construction pass:

```text
source -> ZenUML lexer/parser -> ZenUML semantic artifact
                         |-> detection facts / editor facts / LSP
                         |-> compatibility JSON projection
                         `-> typed layout -> typed SVG
```

The compatibility JSON is a projection for existing callers only. It is never parsed back into a
different family model.

The lexer is a Unicode-aware token scanner. It does not use regular expressions to infer nested
syntax, and it removes the oracle's hidden modifier channel before parsing. The parser consumes
tokens through explicit grammar rules and bounded recursive blocks. Shared Mermaid accessibility
terminals use the common terminal parser, including multiline `accDescr`, so ZenUML does not grow a
second line-oriented directive parser.

## Grammar surface

The oracle grammar and recovery behavior are represented for:

- title, accessibility title/description, comments, and divider notes;
- participant annotations, colors, stereotypes, emoji, widths, aliases, groups, and starters;
- synchronous and asynchronous calls, explicit/implicit owners, nested calls, creation, named
  assignments, returns, return arrows, and expressions/parameters;
- `par`, `opt`, `critical`, `section`, `if/else-if/else`, `while/for/foreach/loop`,
  `try/catch/finally`, and `ref` fragments;
- Unicode identifiers, closed and in-progress strings, exact byte spans, bounded nesting, and
  local error recovery.

The parser keeps facts before and after an invalid statement. Strict semantic/render entry points
return the first structured diagnostic; the editor entry point returns the recovered family facts.

## Support levels

| Surface | Level | Evidence |
| --- | --- | --- |
| Grammar parse and recovery | Implemented against selected source | `zenuml` parser tests and editor corpus |
| Semantic topology and source ranges | Implemented | typed model and `EditorSemanticFacts` tests |
| Headless SVG topology, labels, colors, fragments | Implemented source-derived port | `crates/merman/tests/zenuml_typed_render.rs` |
| Pixel-identical browser geometry | Residual under measurement audit | `docs/alignment/ZENUML_GEOMETRY.md` |
| Historical `3.47.8` to selected `3.50.1` delta | Admitted; compact decision retained | `tools/upstreams/MERMAID_SELECTION_DECISION.json` |
| Future compatible or major candidate | Manual admission only | `.github/workflows/mermaid-admission.yml` and `ZENUML_BROWSER_ADMISSION_PROBES.json` |

Pixel or browser-dependent differences must be recorded as evidence. They must not be hidden by
fixture-specific comparator exceptions.

## Resource contract

ZenUML owns the accounting for participants, groups, statements, retained text, and semantic
nesting, but exposes no family-specific resource knobs. The typed semantic model charges those
values to the shared `max_model_items`, `max_model_text_bytes`, and
`max_model_nesting_depth` budgets before layout. Source, derived layout work, SVG element, and SVG
byte budgets use the same family-neutral contract as every other diagram. This keeps the binding
surface stable as the grammar evolves while still making every added semantic construct
resource-accounted.

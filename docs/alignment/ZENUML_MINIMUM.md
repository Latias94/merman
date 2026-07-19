# ZenUML Family Contract

Merman implements ZenUML as a family-owned grammar and semantic model. It does not translate
ZenUML into Mermaid Sequence actions or Sequence JSON.

## Authority

- Mermaid baseline: `mermaid@11.16.0`.
- Normative companion oracle: `@zenuml/core@3.47.8`, commit `c81406671c0833baebb9fac08a0cbcdc99b3907d`.
- Oracle grammar: `repo-ref/zenuml-core/src/g4/sequenceLexer.g4` and
  `repo-ref/zenuml-core/src/g4/sequenceParser.g4`.
- Candidate: `@zenuml/core@3.50.1`, commit `38404ccc14243ed54ab45b804b2eb6f2ca73af36`.
  The candidate remains a separate admission until the U1 semantic, render, rich-artifact,
  security, and resource gates pass. The latest major `4.2.0` is outside Mermaid's declared
  plugin range and is not an implicit upgrade.

The machine-readable decision is `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`; companion gate
evidence is `tools/upstreams/ZENUML_CORE_ADMISSION.json`.

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
| Grammar parse and recovery | Implemented against oracle | `zenuml` parser tests and editor corpus |
| Semantic topology and source ranges | Implemented | typed model and `EditorSemanticFacts` tests |
| Headless SVG topology, labels, colors, fragments | Implemented source-derived port | `crates/merman/tests/zenuml_typed_render.rs` |
| Pixel-identical browser geometry | Residual under measurement audit | `docs/alignment/ZENUML_GEOMETRY.md` |
| Candidate 3.50.1 deltas | Pending admission | U1 descriptor/gate record |

Pixel or browser-dependent differences must be recorded as evidence. They must not be hidden by
fixture-specific comparator exceptions.

## Resource contract

ZenUML owns structural limits instead of borrowing Flowchart or Sequence limits. Interactive,
Typst, trusted-native, and unbounded profiles populate `max_zenuml_participants`,
`max_zenuml_statements`, and `max_zenuml_fragments`; `max_source_bytes`, `max_label_bytes`, nesting
depth, and `max_svg_bytes` remain the shared outer bounds. All structural and label checks run on
the typed semantic model before layout. The optional fields extend the existing ABI 2 JSON options
without changing the ABI number.

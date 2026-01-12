# ADR 0036: Treemap Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `treemap` diagram is implemented using the `@mermaid-js/parser` Langium grammar:

- Grammar: `packages/parser/src/language/treemap/treemap.langium`
  - header: `treemap` | `treemap-beta`
  - sections: quoted strings with indentation-based hierarchy
  - leaves: quoted strings with `:` or `,` separator and numeric values
  - styling:
    - per-node class selector via `:::className`
    - `classDef` statements for reusable class styles
- Mermaid population and DB behavior:
  - `packages/mermaid/src/diagrams/treemap/parser.ts`
  - `packages/mermaid/src/diagrams/treemap/db.ts`
  - `packages/mermaid/src/diagrams/treemap/utils.ts` (`buildHierarchy`)

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `treemap` parsing in `merman-core` as a handwritten parser that mirrors Mermaid’s
observable behavior:

- Parse header (`treemap` or `treemap-beta`), then statement rows.
- Parse item rows using indentation as the hierarchy key:
  - Section: `"Name"` with optional `:::classSelector`
  - Leaf: `"Name": <number>` (or comma separator) with optional `:::classSelector`
- Parse `classDef` rows and implement Mermaid’s style splitting behavior.
- Convert the flat `(indent,name,type,value,class)` list into a hierarchical tree using a Rust port
  of Mermaid’s `buildHierarchy` algorithm.
- Produce a headless semantic model aligned with Mermaid DB state: `root` tree, preorder `nodes`
  list (with derived recursion `level`), and `classes`.

## Rationale

- The grammar is statement-oriented and indentation-driven; a dedicated Rust parser is simple and
  fast to iterate while we keep up with Mermaid changes.
- The DB logic is lightweight and can be mirrored directly without introducing an additional
  grammar toolchain for this diagram.

## Consequences

- Some Langium tokenization quirks must be preserved (e.g. indentation measured in character count,
  and `NUMBER2` parsing behavior).
- Rendering parity (D3 treemap layout, SVG output) is out of scope for this phase.

## Revisit criteria

If treemap syntax grows substantially or if exact syntax error/span parity becomes critical,
revisit implementing it via the shared lexer + grammar pipeline.


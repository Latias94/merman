# ASCII Graph Parser Padding - Design

Status: Active
Last updated: 2026-05-29

## Intent

Close the remaining non-subgraph graph fixture gaps that are caused by parser/model semantics or
padding-aware layout, before opening the larger subgraph parity lane.

## Problem

The current graph renderer has strong route/label behavior, but several copied `mermaid-ascii`
fixtures still fail for reasons outside complex subgraph routing:

- Mermaid `%%` comments must not create graph statements.
- Bare node definitions must preserve declaration order.
- Explicit labels attached before a bare reference must survive later references.
- `paddingX=` and `paddingY=` fixture directives must affect ASCII render options.
- Backlink routes that depend on shorter Y padding need route-grid spacing parity.

## Scope

- `crates/merman-core/src/diagrams/flowchart`
- `crates/merman-ascii/src/graph`
- `crates/merman-ascii/tests/graph_fixture.rs`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`
- Flowchart/graph tests needed to protect behavior.

## Non-Goals

- Complex nested subgraph parity.
- Multiline node, edge, or subgraph labels.
- New public API unless padding directives cannot be represented safely through existing render
  options.

## Target Fixtures

- `ascii/comments.txt`
- `ascii/preserve_order_of_definition.txt`
- `ascii/explicit_label_after_bare_reference.txt`
- `ascii/custom_padding.txt`
- `ascii/backlink_with_short_y_padding.txt`
- `extended-chars/comments.txt`
- `extended-chars/preserve_order_of_definition.txt`

## Boundary Plan

- Keep comment handling in the flowchart lexer/parser layer.
- Keep node declaration ordering and explicit-label precedence in the flowchart-to-ASCII adapter.
- Keep padding directives as render fixture/options behavior, not as hard-coded fixture names.
- Keep route-grid padding logic local to graph layout/path routing.

## Testing Plan

- Use target graph fixtures as executable regression coverage by moving exact matches into the
  allowlist only after behavior matches.
- Focused gates:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
- Broad gates:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`

## Risk Plan

- Comment lexing can change parser statement boundaries. Add focused parser tests before moving
  fixtures.
- Declaration order can affect existing graph output. Preserve current edge order and only adjust
  node insertion semantics where Mermaid requires it.
- Padding directives in copied fixtures should not leak into normal Mermaid parsing as graph nodes.

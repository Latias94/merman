---
type: "Memory Event"
title: "Verification: U4 Flowchart subgraph TD direction preservation"
description: "Flowchart keeps root TD-to-TB normalization but preserves explicit subgraph direction TD per Mermaid 11.16 parser behavior."
timestamp: 2026-07-09T14:03:51Z
event_kind: "Verification"
---
# Event

U4 Flowchart 11.16 parser slice: Mermaid 11.16 keeps a subgraph-local `direction TD` as raw `TD`
instead of normalizing it to `TB`. Merman now only normalizes `TD` in root graph direction headers;
`DirectionStmt` tokens preserve the explicit subgraph value. Verified with focused core tests for
subgraph direction preservation and root header normalization.

# Impact

This prevents future Flowchart refactors from conflating root direction shorthand with explicit
subgraph direction statements. Golden refreshes for 11.16 should expect current upstream subgraph
metadata to contain `dir: "TD"` where the fixture uses `direction TD` inside a subgraph.

# Citations

- `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/parser/subgraph.spec.js`
- `crates/merman-core/src/diagrams/flowchart/lexer.rs`
- `crates/merman-core/src/tests/flowchart.rs`

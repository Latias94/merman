# HPD-050 - Flowchart Subgraph Builder Frame Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The earlier Flowchart deep-subgraph hardening removed recursive render/layout traversal over nested
subgraphs. A follow-up production panic scan still found three invariant `expect(...)` calls in
`SubgraphBuilder::eval_statements(...)`, the core subgraph membership extraction step. Those calls
are guarded by the explicit stack loop in normal control flow, but they sit on the public
flowchart parse/model boundary and can become input-reachable if future stack invariants drift.

## Changes

- Replaced `expect("frame stack should not be empty")` with a defensive `stack.last_mut()` branch
  that returns the partially accumulated root items if the stack is unexpectedly empty.
- Replaced `expect("current frame should exist")` with an `if let Some(frame)` guard for ordinary
  statement item collection.
- Replaced `expect("finished frame should exist")` with a defensive `stack.pop()` branch.
- Preserved normal subgraph member ordering, direction inheritance, id/title parsing, and nested
  subgraph evaluation behavior.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core flowchart` - passed, `98` tests run.
- `rg -n 'frame stack should not be empty|current frame should exist|finished frame should exist' crates/merman-core/src/diagrams/flowchart/subgraph.rs` -
  no matches.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `862`
  lines parsed.

## Boundary

This is a core Flowchart parser/model panic-surface guard. It does not change Flowchart grammar,
subgraph member ordering, renderer layout, SVG structure, theme CSS, root viewport formulas,
stored baselines, or Flowchart residual classification.

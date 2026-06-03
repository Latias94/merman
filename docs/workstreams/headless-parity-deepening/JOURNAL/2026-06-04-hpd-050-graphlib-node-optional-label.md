# HPD-050 - Graphlib Node Optional Label Seam

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The Architecture root residual path is currently bounded by negative evidence: single-constant
group padding, font-family, and service-label lookup changes are not safe production fixes. The
next useful HPD-050 work therefore returned to the Dugong/Graphlib source-audit lane, where direct
Graph API behavior can be tightened without overfitting SVG root residuals.

## Source Finding

Pinned Graphlib `test/graph-test.js` has a small but important node-label seam:

- `setNode("a", undefined)` clears the node value while keeping the node in the graph.
- `node("a")` also returns `undefined` for a missing node, so callers distinguish these states with
  `hasNode(...)`.

Rust cannot mirror JS `undefined` directly, but the existing `Graph<Option<N>, ...>` shape is the
same seam used by Graphlib JSON: a missing node returns `None`, while a present node with an
upstream `undefined` label is represented as `Some(None)`.

## Outcome

- Added direct `graph_core_test` coverage for clearing an optional node label without removing the
  node.
- The test also locks the missing-node versus present-undefined distinction:
  `node("missing") == None`, while `node("a") == Some(&None)` after the explicit clear.
- Updated the Graphlib upstream coverage ledger to map the relevant `setNode` and `node` source
  cases to the new Rust test.
- No production Graph implementation change was needed.

## Verification

- `cargo nextest run -p dugong-graphlib set_node_with_optional_label_can_clear_label_without_removing_node`
- `cargo nextest run -p dugong-graphlib` - passed, `96` tests run.

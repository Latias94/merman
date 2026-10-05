---
type: "Work Progress"
title: "Mermaid 12 container option boundaries"
description: "Verified preset and container option ownership, source port placement, and remaining NetworkSimplex work."
timestamp: 2026-09-20T06:55:10Z
record_id: "a75a923d906a4794b24cd68c3a98df29"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "bb5bcff54"
supersedes: "f837cb6d1f7d4f868f77523a709d1be1"
---

# Summary

The full Mermaid 12 goal remains active. Commit `bb5bcff54` completes the root/container
preset boundary and its geometry correction, not all of U5. The selected bundle, generated
production defaults and fixtures still identify 11.17.2. All prior implementation and staged
runtime evidence in the superseded checkpoint remains valid. No remote publication occurred.

# Details

- `ElkInputNode.nested_options` replaces `nested_spacing_base`. The native importer now reads
  a container's own resolved options; Mermaid policy is supplied by the adapter. Root layering,
  model-order and routing do not silently become child settings. Direction and hierarchy still
  follow the source compound-layout rules. Explicit nested random-seed tests retain operation
  seed semantics.
- `LayoutOptions.container` distinguishes container placement from root placement. Default is
  BK/BALANCED/DEPTH_FIRST. modelOrder/depthFirst use NetworkSimplex at the root and BK inside
  containers unless explicitly overridden. Containers use base spacing 24, padding 24,
  node spacing 50, edge-edge spacing 20, between-layer edge-node spacing 30. Root and containers
  receive surrounding port clearance 12.
- Two existing compound geometry assertions were updated from actual installed elkjs 0.9.3
  output using the target Mermaid root/container options and their exact measured test inputs.
  Development collector: `target/mermaid-12-admission/compound-options.cjs`; outputs:
  `median-elkjs.json`, `thickness-elkjs.json`. The collector places edges on their LCA and uses
  nonempty text with explicit label sizes. No published baselines were relabeled.
- The 0.4px parallel-edge endpoint discrepancy was a native node-spacing omission. It is NOT
  proportional padding compression: the initial hypothesis was disproved by a setter trace.
  `InsidePortLabelCellCreator.setupNorthOrSouthPortLabelCell` reserves vertical label-port
  spacing even for empty cells; `CellSystemConfigurator.updateVerticalInsidePortLabelCellPadding`
  subtracts this and label-cell spacing from side-port surrounding margins. The Rust placement
  now applies that source rule. Container endpoints 103.4/92.6 and label centers match elkjs.
- Verification: 392/392 kernel+adapter tests passed (nextest
  `43a0eb60-8d61-47d1-92f0-6676b1400da0`); six focused renderer option tests passed
  (`ede250a8-dd38-4c20-8b88-cb2e4aea460f`). The extended nested option test initially used the
  adapter field name in the kernel fixture, then was corrected to `layering_strategy`; its
  final run passed (`5aa9301f-373a-4dac-b63a-92945b92524c`). Formatting/diff whitespace checks passed.

## Independent review

`/root/review_container` completed a read-only correctness review. One real P2 remains:
Mermaid render.ts sets `nodePlacement.networkSimplex.nodeFlexibility=PORT_POSITION` on each
container NODE. ELK `NodeFlexibility.getNodeFlexibility` checks that node property before the
parent graph's `nodeFlexibility.default`. This must not be modeled as the default flexibility
of all children inside the container. Native input/LNode and NetworkSimplex currently lack this
behavior. The review also confirms that target generated-default promotion is required before
Engine-driven presets work: old generated placement/alignment values otherwise look explicit.
This was a focused review, not final implementation-wide approval.

# Next Action

1. Complete node-level PORT_POSITION through native input, LNode, adapter container construction
   and NetworkSimplex. Inspect the existing simplified placer against selected Java source;
   port source auxiliary corner/port constraints and actual position application. Source methods:
   `prepare`, `transformLayer`, `transformFixedOrderNode`, `transformPorts`, `transformEdge`,
   `applyPositions`, `isFlexibleNode`. The source also has north/south and in-layer auxiliary
   edges and straight-path pre/postprocessing that the current simplified implementation omits.
   Do not substitute fixed ports or conflate node properties with child graph defaults. Use a
   distinguishing elkjs sample with multiple movable container ports and root NetworkSimplex.
2. Continue U5 frame equalization/terminal jog straightening, U3 measured backend dispatch, and
   U6-U8 target-reachable algorithms. U1 complete companion graph/receipt and U2 target theme
   evaluator/default promotion remain open, as do U9-U15 native families, editors, feature/legal
   closure and final integration. Remove the documented temporary Swimlane default bridge on
   target default promotion. Follow the accepted plan, not a reduced subset.
3. Serial Cargo, normal target, focused reruns only. No running builds remain at this checkpoint.
   Do not repeat the already successful signature audit for an unchanged reference graph.

# Citations

- [Accepted implementation plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Previous verified implementation checkpoint](2026-09-20T062227Z-mermaid-12-verified-layering-and-staged-runtime-projections-f837cb6d1f7d4f868f77523a709d1be1.md)

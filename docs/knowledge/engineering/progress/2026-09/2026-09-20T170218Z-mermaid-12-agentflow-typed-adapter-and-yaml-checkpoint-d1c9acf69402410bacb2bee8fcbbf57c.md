---
type: "Work Progress"
title: "Mermaid 12 Agentflow typed adapter and YAML checkpoint"
description: "Agentflow now uses typed graph projection, shared controlled YAML, grouped endpoints and borrowed statement splitting; family admission remains incomplete."
timestamp: 2026-09-20T17:02:18Z
record_id: "d1c9acf69402410bacb2bee8fcbbf57c"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "5fab6c839"
supersedes: "b8461d837149429d90a79c4707f0c076"
---

# Status

GOAL U1-U15 remains active and incomplete. Target alpha.7 is main unpublished work plus Mermaid 12
alignment. Package versions remain alpha.6; the selected production reference is still 11.17.2 pending
atomic U15 promotion. No release, push, PR or merge. FFI, built Web WASM, Playground and golden fixtures
remain required, as described in the plan and previous checkpoint.

# Implemented Since the Previous Checkpoint

Commit `5fab6c839` removes Agentflow's JSON round-trip into FlowchartModel. Nodes, marker/stroke enums,
subgraphs and render context are constructed directly; explicit edge-id provenance is retained and the
adapter is infallible. The caller no longer turns a serialization failure into a layout error.

Borrowed top-level statement splitting handles semicolons, comments, quoted labels, metadata blocks and
Unicode. Grouped endpoints expand in upstream source/target order, with the explicit id attached only to
the last source / first target pair. Chained groups, Unicode edge ids, extended arrows, dotted reference
lengths and reference labels follow the pinned parser. Header TD normalizes to TB, and a directionless
header followed by a semicolon is rejected as the pinned lexer does. Expanded edges obey secure
maxEdges, with periodic cancellation checkpoints during node/edge expansion.

The bespoke metadata YAML approximation was deleted. Agentflow now calls the existing controlled
inline/YAML parser, retaining nesting/materialization limits and cancellation. The source-backed retry
removes only trailing block-YAML commas after the first parse fails; quoted strings, flow collections,
comments and literal scalar content retain their meaning. ParseFailure distinguishes syntax failure
from cancellation and propagates cancellation through the existing OperationControlResult boundary.

# Verification

- Agentflow core: 17 tests passed after canonical YAML integration, run
  `041fa58d-0b14-4caf-aef8-d80513f7eb5b`; the subsequent new nested YAML/literal-comma test passed as
  `cb82d555-c9ff-4d81-bff6-1e59de84923c`. Earlier grouped endpoint/edge-limit run:
  `0875762b-abd1-4448-a5c7-795cfe7e9f43`.
- Renderer integration: both Agentflow tests passed, run `3d073342-7faf-454e-bad9-7bec92675085`,
  exercising ELK and Dagre collapse routes and the typed adapter.
- Scoped Rust 2024 formatting and staged diff checks passed.
- The pinned Mermaid12 browser probe in `target/mermaid-12-admission/agentflow-parser-oracle.cjs/.json`
  now includes grouped endpoints, semicolon statements and nested YAML with trailing commas/literal
  content. Source/target edge order, generated and explicit ids, lengths and domain metadata agree
  with the focused Rust expectations. Probe outputs are exploratory evidence, not admitted goldens.

# Next Action / Remaining Scope

1. Complete Agentflow anonymous flow IDs and container completion/declaration ordering, full
   style/class/click/linkStyle propagation, cyclic containment handling and diagnostic source spans.
2. Complete shape registry rejection and metadata/render propagation. Current shape diagnostics are
   semantic payloads, not yet fully projected into the shared editor warning surface. Complex quoted
   edge-label forms and malformed trailing metadata still require source-backed parser coverage.
3. Review the new parser/adapter changes independently and then complete family-local SVG/DOM admission;
   do not label the entire family complete based on these focused tests.
4. Continue Usecase residuals and U9-U15 integration, including grammar, FFI, actual WASM/Playground,
   feature/license closure, target goldens and atomic reference/version promotion. Remove transitional
   Agentflow/swimlane defaults when generated Mermaid12 config is promoted.
5. No Cargo workload remains active. Reuse target, serialize Cargo, and preserve the unrelated
   `crates/merman-core/src/tests/flowchart.rs` formatting diff (not staged or committed).

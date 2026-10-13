---
type: "Work Progress"
title: "Mermaid 12 Agentflow container and shape convergence checkpoint"
description: "Source-backed container completion, collapse cycles, shared shape registry and strict metadata validation; family admission remains incomplete."
timestamp: 2026-09-20T17:28:28Z
record_id: "4a08778c9aa445efba94ac3c650e5b69"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "ac85094e6"
supersedes: "d1c9acf69402410bacb2bee8fcbbf57c"
---

# Status

GOAL U1-U15 remains active and incomplete. Target alpha.7 is main unpublished work plus Mermaid 12
alignment. Package versions remain alpha.6; selected production reference remains 11.17.2 until atomic
U15 promotion. No release, push, PR or merge. FFI, actual Web WASM, Playground and authentic target golden
fixtures remain required. Do not mark the goal complete on the strength of family-local tests.

# Implemented

Commit `ac85094e6` replaces eager Agentflow container membership with pending contexts completed at
`end`, matching `agentflow.jison` and `agentflowDb.addSubGraph`. Nested graphs enter the semantic model
inside out, every completed flow advances the anonymous `subGraphN` counter, duplicate IDs merge into
the first completed record, and empty later titles do not erase previous titles. Member lists preserve
statement reductions (later chain endpoint groups precede earlier groups), deduplicate at completion,
and exclude existing ownership, self membership and global exemptions. Global blocks release direct
members without recursively exempting nested children. Local direction and inheritDir remain distinct.

Agentflow computes collapse descendants in source completion order and parent admission in reverse
completion order. It preserves authored cyclic membership in semantic JSON, emits the upstream
CONTAINMENT_VIOLATION warning, and removes only refused nesting from the layout projection. The same
collapse map drives edge redirection and shared layout filtering. This fixes the independent review's
counterexample where two mutually containing collapsed flows both disappeared: the first root A now
remains visible with B/a/b mapped to it, as the pinned runtime does.

Container colors use preorder ordinals recovered from the admitted parent forest, passed as parser-owned
FlowchartRenderContext facts instead of model-array indices. Ordinal and collapse maps stay out of
compatibility JSON. Source and target endpoint metadata share one dispatch path. Illegal content after
metadata is rejected rather than discarded. Multiline accDescr uses the upstream plain-text-until-}
rule, including unmatched quotes/brackets. Grouped node declarations now have periodic cancellation
checkpoints; deduplication no longer scans the pending member vector on every insertion.

Commit `9639c3c72` moves the shape registry out of Flowchart's shape-data handler into shared
`diagrams/shapes.rs`, including the ten additional Mermaid 12 keys (177 total). Flowchart retains its
public catalog functions. A native Declaration separates shorthand shape, authored metadata shape,
resolved metadata and metadata source span. Ordinary vertices reject unknown shape names, uppercase or
underscore names, and non-string truthy shapes; domain aliases validate before resolution. Existing
container/connector/edge metadata takes its own dispatch path before vertex validation. Known removed
and unsupported shapes retain semantic shape identity and source-backed rendering fallback. Null,
false, zero and empty-string metadata shapes follow the pinned JavaScript truthiness boundary.

# Evidence

- Core container suite: 25 passed, nextest `e8d49365-453d-484b-aacd-dc21b6c799f7`.
- Accessibility lexical boundary: passed, `da1096dd-6e34-4b10-8999-f52edddb5784`.
- Collapsed cycle context assertions: passed, `8bf195e1-c537-4dd5-95fe-1ccd34257150`.
- Latest core suite after shared registry/declaration changes: 37 passed (27 Agentflow plus 10
  existing Flowchart shape tests), `94e1572c-b8cd-409b-966f-a98fecac9347`.
- Latest Agentflow SVG integration: 3 passed, `71dcf7b1-4e47-4e25-af10-6e261f20bfb6`, covering ELK and
  Dagre default/collapse, authored loops, connectors, and both expanded/collapsed containment cycles.
- Independent parser review confirmed endpoint dispatch, metadata-tail and accessibility issues;
  all were fixed. A subsequent review found the two-collapsed-container disappearance; it was fixed
  with the shared Agentflow collapse map and verified on both backends. Final shape review found no
  new definite issue and independently confirmed all 177 registry entries and source behavior.
- Pinned Mermaid 12 browser oracle probes:
  `target/mermaid-12-admission/agentflow-containers-oracle.cjs/.json` and
  `target/mermaid-12-admission/agentflow-shapes-oracle.cjs/.json`. They establish anonymous IDs,
  membership/chain order, duplicate/global behavior, collapsed-cycle visibility, strict shape errors,
  falsey-shape acceptance and container metadata dispatch. These remain exploratory evidence,
  not admitted or regenerated goldens.
- Scoped Rust 2024 formatting and staged diff checks passed. Cargo ran serially with shared target.

# Next Action / Remaining Scope

1. Complete Agentflow style/class/classDef/click/linkStyle syntax and render metadata propagation.
   Keep the semantic projection presentation-free; do not route the entire family through a renamed
   Flowchart parser or add fabricated metadata keys to carry renderer state.
2. Add diagnostic source positions and shared editor warning projection for shape/containment
   diagnostics, including BOM/CRLF/frontmatter and UTF-16 remapping. Shape parse failures now identify
   the metadata block, but structured family diagnostics still lack source positions.
3. Finish source-backed parser/metadata/quoted-edge-label boundaries and family-local SVG/DOM admission.
   Connector label/metadata update behavior still needs full source convergence. No claim of complete
   Agentflow parity is justified yet.
4. Continue Usecase residuals and U9-U15 integration: grammar, FFI, built WASM/Playground, feature/license
   closure, genuine target goldens and atomic reference/version promotion. Remove transitional
   Agentflow/swimlane defaults at generated Mermaid 12 configuration promotion.
5. No Cargo process remains active. Preserve the unrelated `crates/merman-core/src/tests/flowchart.rs`
   formatting diff; it was neither staged nor committed. Continue on refactor/mermaid-12-alignment.

# Source References

Pinned Mermaid commit `98a0945418c76238f15df2afaddbba4272656c3b`:
`packages/mermaid/src/diagrams/agentflow/{agentflowDb.ts,parser/agentflow.jison,shapes.ts,colorSlots.ts}`
and `packages/mermaid/src/rendering-util/rendering-elements/shapes.ts`.

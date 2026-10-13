---
type: "Work Progress"
title: "Mermaid 12 explicit container projection and alpha.7 release scope"
description: "Container metadata and title constraints reach native providers; sizing lifecycle and renderer integration remain in progress."
timestamp: 2026-09-20T09:53:35Z
record_id: "37c5db64031849829fe210f69a874869"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "78c114cce"
supersedes: "5a66d2154b834776b5a5589bdfbaa008"
---

# Summary

Full U1-U15 / R1-R12 goal remains active and incomplete. This continuation made concrete
progress in `78c114cce` (container metadata, option provenance, source sizing constraints).
Production defaults/reference selection remain 11.17.2; do not promote the staged bundle yet.
No push, PR, release, or goal completion.

Maintainer explicitly set and confirmed the release scope: **v0.8.0-alpha.7 = unreleased main
changes + this Mermaid 12.0.0 alignment**, compared with published **v0.8.0-alpha.6**. The plan
now records this. Keep development package versions at alpha.6 until the final coordinated
version projection/changelog step; independently versioned support packages remain separate.

# Details

## Container model and resolver

- Core `FlowSubgraph.metadata: Option<Value>` now preserves the parser's existing shallow merge.
  None is omitted from serialization. Unknown fields and invalid algorithm values survive until
  the layout boundary. All external struct literals were updated with None.
- Flowchart group measurement resolves exactly the eight Mermaid container names through
  `Algorithm::from_container_name`; no trimming, aliases, or case folding. Root allowlists are
  unchanged. `Node.container: ContainerNodeOptions { algorithm, padding }` carries a valid
  metadata algorithm plus Mermaid's raw node padding (not ELK content padding).
- New `container.rs` holds `ContainerMode::{Ordinary,Explicit(Algorithm),Cleared}`. A valid
  explicit algorithm suppresses `node.direction`, even after it is cleared by crossing edges.
  Nonempty explicit groups always separate. Standalone explicit Layered defaults Right;
  explicit MrTree defaults Down (its source Undefined direction).
- Before planning scopes, graphs with valid metadata normalize cross-hierarchy endpoint chains
  through the endpoint-inclusive common ancestor. Same-parent edges leave metadata intact,
  including g->outside when both are root children. g->own-child clears g. Cleared metadata
  uses IncludeChildren and ordinary title minima, spacing50/base24, but **padding12**, since
  upstream deletes its special padding without restoring ordinary24. Direction-only groups
  preserve direction; Flowchart continues providing their preexisting include policy.
- Explicit presets project minimum(labelW+2*nodePad,labelH+30), top(labelH+15), other padding15,
  aspect2, center/top content alignment and expandNodes. Rectpacking uses padding10, minimum
  height labelH+20, and the shared target preset (aspect1.6, spacing15, trybox/compaction/etc).
- Nonempty groups consume completed child extents; merged groups import with zero dimensions.
  Stale measured group dimensions are no longer maxed back into their computed extent.
- Diagnostics now reject non-Layered scopes and any graph requiring more than one independently
  scheduled scope (`SeparateScopesDiagnosticsUnsupported`). They do not return a zero-size
  group shell with missing children. Single-scope cleared metadata uses normal diagnostics.

## Source sizing lifecycle

In EPL `merman-elk-layered`, LayeredOptions now has `node_size_minimum: Option<LSize>`,
`node_size_default_minimum` (default true, nonpositive axes become20),
`node_size_include_labels`, and horizontal/vertical ContentAlignment (Start/Center/End).
`include_inside_top_center_label_minimum` is fallible and implements the reachable single
inside-top-center title's symmetric label grid. Standalone scope adapter calls it explicitly;
merged compound importer calls it for its owner label.

SeparateChildren graph resizing runs after the complete flat pipeline/direction restoration.
IncludeChildren retains the scheduled HierarchicalNodeResizer. Growth updates graph.offset and
existing east/south hierarchy ports. `LGraph::exported_root_size()` implements the final exporter
minimum clamp without a second content shift; adapter scope sizing now calls it.

Source peculiarity verified by elkjs: DOWN+INCLUDE, minimum120x80, H_CENTER/V_TOP and a40x20
single child produces parent120x120 and child(0,50). Do not "correct" this into120x80 by intuition.
Raw minima and computed label minima reject nonfinite values through typed NodeSizeError.
The full/partial pipeline preparation checks nested options; resizer also validates at execution.

Flat scopes retain whether the child provider left size constraints active. Radial writes sizes
directly and retains constraints; other reachable providers fix them through ElkUtil.resizeNode.
Stress's initial Force export clears them before majorization, so final Stress output must NOT
reapply the old minimum. Box sorts incoming children before resetting active child dimensions
to the effective minimum; its Rectangle.minimum_size represents that source operation and
rejects negative/nonfinite effective dimensions. Radial under Box can therefore have a frame
smaller than its child extents, matching upstream rather than hiding this behavior.

## Evidence and review

- Actual explicit metadata input/output: `target/mermaid-12-admission/explicit-container-oracle.{cjs,json}`.
  Root Box isolates one group, group children a40x20,b60x30 with an inline zero-size edge label,
  title30x10 or300x10, Mermaid node padding15. Sixteen cases cover all eight container algorithms.
  Committed tests check group extent and local children, including expanded widths and metadata
  overriding an authored Up direction. No fixture was generated from Rust output.
- `cargo nextest run -p merman-layout-elk -p merman-elk-layered ...`: **455 passed**, run
  `3b4b69b6-7a3f-4458-87a7-0b3a81ab77a5`, before final review guards.
- Final changed sizing/diagnostic/invalid-input tests: **11 passed**, run
  `f8b617d1-05fc-4f6c-b845-8d77d17fcd7a`. Two unused test Result warnings were then fixed with
  unwrap; no behavior changed. Renderer+ASCII tests compilation subsequently passed.
- Core metadata and renderer parse/measurement tests: **4 passed**, run
  `518b7996-3c04-4ca5-8c13-3a36455a2476`.
- `cargo check -p merman-render -p merman-ascii --tests --features merman-render/layout-elk`
  passed; fmt/diff checks and `python scripts/verify-third-party-licenses.py` passed. No new
  dependency or feature closure. All ELK sizing translations stay inside the EPL crate.
- Metadata worker initially authored invalid nested-inline-brace syntax in tests. Target Jison
  also closes shapeData at the first unquoted }, so tests now use genuine multiline YAML maps
  for shallow nested merge and an array for a non-string algorithm. Parser was not broadened.
- Independent correctness reviewer `/root/review_container_projection` found diagnostic child
  loss and nonfinite Layered minima; both corrected with focused tests. Its earlier Box negative
  minimum concern was already fixed during review. Reviewer ran no Cargo and edited nothing.

# Next Action

Continue **provider node micro layout and sizing state through non-Box parents**. The explicit
container oracle currently proves Box-parent behavior, not arbitrary nested combinations. Force,
Stress, MrTree, Radial and Rectpacking invoke NodeMicroLayout; Box uses ElkUtil.resizeNode only;
SPOrE only calculates margins. Retained Radial constraints must be consumed correctly by each
parent, and fixed children must not regain active constraints. Current flat materialization
passes size_constraints_active through NodeContext, but only Box consumes its effective minimum;
Rectpacking's ordinary non-Box branch and other providers still need real source micro layout.
Layered materialization also needs the corresponding input child constraints/label sizing
contract for separately completed scopes, rather than treating every measured child as fixed.
Avoid a broad general ELK option framework: port the reachable source operation into EPL.

Then finish remaining hierarchy boundary routing, renderer root registration and true missing
sections handling (empty point chains are not real two-point sections). Current cross-provider
boundary segments still return UnsupportedCrossProviderEdge, never silently substitute Layered.
Root elk.layered and elk.radial remain unregistered, as in Mermaid's root registry.

Preserve the full remaining plan: defaults/themes and bundle promotion; State/Requirement/Mindmap
backend integration; U5 remaining postprocessing; current-family geometry/appearance;
Agentflow/Usecase; editor/Tree-sitter/playground/bindings; default feature and EPL distribution
closure; genuine target fixtures/final admission, required integration, size/performance, final
review. Final release comparison includes all main changes since alpha.6. Do not mark the goal
complete from these adapter tests.

Legacy memory rollups remain unadopted; capture shards without attempting another migration or
replacing hand-maintained root views.

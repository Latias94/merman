---
type: "Work Progress"
title: "Mermaid 12 root provider registration and graph-family missing sections"
description: "Root ELK names execute real providers; Class and ER draw missing sections and Class cardinalities use clipped geometry."
timestamp: 2026-09-20T10:32:17Z
record_id: "c3254522c0a0493ebc49f583bcdcbbc6"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "82ae7ea7f"
supersedes: "4a45d432eb464cf0a1bf9b4118ef2e92"
---

# Summary

Goal remains active and incomplete. Commit `82ae7ea7f` integrates the native root providers
with the existing Flowchart/Class/ER dispatch and adds Class/ER missing-section rendering.
No default/bundle promotion, release, push or PR. Release target remains alpha.7 = unreleased
main + Mermaid12 alignment, compared with alpha.6. Keep package versions unchanged until the
final coordinated version/changelog step. U13/U15 explicit FFI/Web/Playground/golden scope remains.

# Details

- `layout_backend::ElkRootAlgorithm` owns the exact root allowlist independently of the optional
  adapter dependency: elk, elk.stress, elk.force, elk.mrtree, elk.sporeOverlap, elk.box and
  elk.rectpacking. No trimming/case aliases; elk.layered and elk.radial remain unregistered.
  `elk_options::layout_options` maps it to the real adapter Algorithm. Capability planning and
  presentation aspect selection recognize every registered name. Missing compiled loaders use
  Dagre; a registered backend denied by host policy returns MissingCapability without fallback.
- Class/ER retain empty raw provider point vectors. Paint derives a clipped center-to-center
  path, emits linear curves and puts main labels at the clipped midpoint. True two-point
  sections retain their previous path. Class content bounds and ER precomputed bounds include
  the same main-label geometry. Shared `elk_geometry::missing_rect_section_points` is a small
  rectangular-node helper; ClassDB emits even lollipop interface nodes as transparent rects.
- Class cardinality metadata previously disappeared because layout had no path to place it.
  `class_layout_from_elk` now uses the same derived clipped geometry for terminal placement,
  retaining raw points empty. Source `utils.calculatePoint` throws if the line is shorter than
  the required25+marker distance. Missing-section cardinalities now return InvalidModel in
  that case instead of silently dropping labels. This follows the pinned source call chain;
  no new browser oracle was captured for that error. The success test uses eight classes so
  both Box and Rectpacking produce a long enough path; the short two-class case tests the error.
- Worker `/root/class_er_missing_sections` implemented the main paint branches. Root reviewed
  and added shared layout geometry/cardinality handling, root registration and capability tests.
  This commit still needs the goal's independent review; prior reviewer covered only the
  previous Flowchart missing-section commit. Do not imply this larger change has that receipt.

Validation (serialized Cargo, normal target):

- Root dispatch/provider/policy unit filter, layout-elk:4 passed,
  run `0c951a49-832b-45f1-a2a3-2ff769e30370`.
- Lean dispatch and alias fallback unit filter:3 passed,
  run `846a5721-aea3-4e52-8ace-9b8a90801d73`.
- Final Flowchart/Class/ER integration tests filtered by elk:11 passed,
  run `4df3169c-d84a-407a-8d19-57fd71dd85a2`. Includes distinguishing geometries for all seven
  roots in Flowchart, existing family ELK regressions, missing paths/main labels/cardinalities,
  and source short-cardinality error. No claim of a full family/theme/algorithm matrix.
- Final no-default-features cargo check, scoped cargo fmt check, git diff check and third-party
  license verification passed. No Cargo processes remain.

# Next Action

Continue cross-provider hierarchy boundary handling and group-frame geometry. In
`merman-layout-elk/src/lib.rs::materialize_flat_scope`, cross-provider boundary segments still
return UnsupportedCrossProviderEdge. Inspect the actual ELK recursive engine and provider
handling (and source errors) before changing this; never fabricate routes or swap algorithms.
Flowchart's endpoint cutter still lacks the full Mermaid12 group-frame sanitization changes.
Class/ER real routed edges also retain prior curve behavior pending their full family delta pass.

Keep the entire remaining U1-U15 scope: source-backed defaults/themes and State/Requirement/
Mindmap ELK integration; U5 postprocessing; U9/U10 family geometry/appearance; Agentflow/Usecase;
editor/Tree-sitter/FFI/Web WASM/built Playground; default and lean feature closures/licenses;
genuine target goldens, selected bundle admission, final required integration/size/performance
and independent review. Production references remain11.17.2 and must not be relabeled as12.

# Citations

- `docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md`
- Previous immutable record `4a45d432eb464cf0a1bf9b4118ef2e92`.
- Mermaid12 source: `rendering-util/layout-algorithms/elk/{algorithms.ts,render.ts,geometry.ts}`,
  `rendering-util/rendering-elements/edges.js`, `utils.ts`, `diagrams/class/classDb.ts` under
  `repo-ref/mermaid/packages/mermaid/src/`, commit98a0945418c76238f15df2afaddbba4272656c3b.

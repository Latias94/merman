---
type: "Work Progress"
title: "Mermaid 12 native Usecase parsing and ELK default closure"
description: "Native Usecase semantics and editor policy are integrated; rendering and overall Mermaid 12 admission remain incomplete."
timestamp: 2026-09-20T12:25:42Z
record_id: "05764f346b3b4026a526866ddfb1d777"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "8d937ba97"
supersedes: "c3254522c0a0493ebc49f583bcdcbbc6"
---

# Summary

GOAL remains active and incomplete. Target is alpha.7 = unreleased main plus Mermaid 12 alignment.
Versions remain alpha.6 until the final coordinated bump. No publication, push, PR or merge.

# Details

- `8d937ba97` adds a native Usecase lexer, line grammar, deferred semantic resolution and ordered
  JSON object decoding under `crates/merman-core/src/diagrams/usecase`. It covers actor variants,
  implicit endpoints, declaration merging/conflicts, boundaries, relationships and markers,
  notes, JSON, metadata, classes/styles, accessibility and parser-backed editor facts.
  Model enum fields are typed; reverse arrows preserve written source/target order.
- Detection requires whitespace or end after `usecase-beta`. The typed model is registered and
  resource accounting is connected. `metadata: None` intentionally withholds SVG admission;
  render and ASCII dispatch explicitly return UnsupportedDiagram until their real implementations
  exist. Do not mistake this parser milestone for completed U12.
- Added `usecase_identifier` to the existing rename-policy descriptor and regenerated Rust/Web
  projections with `gen-editor-language-contract`. All addressable facts, including explicit
  edge IDs and JSON declarations, use the policy. Its validator reuses the native lexer and
  rejects Unicode/hyphens/reserved tokens while allowing digit-prefixed identifiers.
- Worker `usecase_native_parser` and its lexer worker completed. Root integrated the family,
  transport-neutral rename policy, common-DB sanitation, detector boundary and public API tests.
  This larger parser change still needs independent source/correctness review before final
  admission. In particular, inspect resource/cancellation behavior and JSON nesting acceptance;
  the JSON decoder currently uses serde_json's normal recursion limit.
- `97fbe3b13` changes default compiled product closures: facade `complete-svg-elk`, CLI includes
  layout-elk, Rustdoc svg+Cytoscape+ELK with math opt-in. `ceeec5f00` fixes their documentation
  and regenerates THIRD_PARTY_NOTICES through the existing Python generator. The default config
  itself remains the selected 11.17.2 projection until U15; compiled availability is not baseline
  promotion. Existing staged Mermaid 12 config/theme files remain in target/mermaid-12-admission.
- `eceb8740e` corrects the earlier `396421760` hand-edited generated config: restores the coherent
  active descriptor projection, architecture compatibility fallback and explicit Swimlane
  precedence test. Never independently change default_config.layout again; promote complete
  generated config/shape/themes with the descriptor.
- `c4b420e57` corrects `e968dcbe7`'s speculative loop patch. The original intersection already
  provides the nonzero same-center endpoint. Forced extra endpoints broke touching rectangles.
  Restore the upstream short-route fallback and consecutive dedup, with coincident, touching,
  and tiny-node regression assertions. The earlier reviewer finding was not reproducible for
  normal nonzero rectangles and should not be carried forward as a confirmed bug.

Validation, serialized Cargo in the regular target directory:

- Core Usecase/public-route/rename/Swimlane filter: 19 passed, run
  `9d4a16c4-d94d-42de-ad40-0b843cf89e7e`. Public test verifies frontmatter/Chinese label byte spans,
  semantic-vs-typed JSON agreement, declarations/references and rename policies.
- Family catalog identity/order consistency: 1 passed, run
  `14e67c47-ae56-4587-8cfc-6ec4221518c4`.
- ELK geometry regression: 1 passed, run `2d445a18-93c6-4fa0-882e-a8b935bae821`.
- Core, render(layout-elk), ASCII checks passed; editor-language generation/freshness and license
  generation/freshness passed. Focused rustfmt and git diff whitespace checks passed. No complete
  workspace/family/browser/golden admission was claimed or run.
- A small temporary browser probe used the already verified Mermaid 12 runtime and cached
  Chrome headless shell 151.0.7922.77. `target/mermaid-12-admission/usecase-parser-oracle.{cjs,json}`
  records forward declarations/reverse arrows and two rejected cases (notes cannot target JSON;
  include requires usecase endpoints). This is bounded observational evidence, not a full
  differential suite or a promoted fixture. Initial Chrome launch failed because only headless
  shell was cached; selecting headless:'shell' resolved it without installation.

# Next Action

Implement Usecase measured graph, layout adapters and SVG from the pinned usecaseDb/renderer and
shape sources; replace the explicit unsupported render branch and admit metadata only after
source-backed family tests. Reuse GraphLayoutSelection and operation-owned ELK seed/work meter;
the core model is stable. Fields include nodes/boundaries/relationships/notes/json_nodes/class_defs,
title/acc_title/acc_description. JSON nodes retain property_order by RFC6901 pointer.

Then continue Agentflow, missing existing-family ELK paths, shared appearance/theme work, U13
FFI/Web WASM/Playground/editor exposure, and U15 authentic golden/bundle/version promotion.
The earlier staged-runtime evidence remains valid; do not redo package authenticity acquisition.

# Citations

- [Implementation plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Previous checkpoint](2026-09-20T103217Z-mermaid-12-root-provider-registration-and-graph-family-missing-sections-c3254522c0a0493ebc49f583bcdcbbc6.md)
- Pinned Mermaid source: `98a0945418c76238f15df2afaddbba4272656c3b` under repo-ref/mermaid.

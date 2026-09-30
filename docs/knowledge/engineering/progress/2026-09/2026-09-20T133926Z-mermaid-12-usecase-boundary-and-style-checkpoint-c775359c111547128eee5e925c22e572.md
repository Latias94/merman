---
type: "Work Progress"
title: "Mermaid 12 Usecase boundary and style checkpoint"
description: "ELK centered ports, Dagre isolated boundary extraction and label styles verified; family admission remains open."
timestamp: 2026-09-20T13:39:26Z
record_id: "c775359c111547128eee5e925c22e572"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "14d2ca55d"
supersedes: "f9754c99f40a41a99bf90e9d6cdc505b"
---

# Status

GOAL U1–U15 remains active and incomplete. The target is alpha.7 = unpublished main + Mermaid 12.
Package versions stay alpha.6 and the selected reference stays 11.17.2 until atomic U15 promotion.
FFI, actual Web WASM, Playground and golden refresh remain explicitly required by U13/U15.
No push, PR, publication or merge. The predecessor contains the prior pipeline and source identities.

# Implemented

- `795fdbd75`: transports optional node-level `PortAlignment` through both ELK adapter importer
  paths into the existing layered spacing implementation. Usecase ellipse/business nodes choose
  Center, matching `spreadPorts`; other families keep None. Constructors updated without new
  dependencies or features.
- `14d2ca55d`: extracts isolated one-level Dagre boundaries into child graphs, preserving Graphlib
  insertion order, internal direction (`TB` parent -> `LR`, every other parent -> `TB`), inherited
  nodesep and ranksep + 25. All edges, including note connectors, determine external connectivity.
  Projects source title shifts, actual boundary geometry, painted bounds and `positionNode`/`diff`
  transforms. The implementation is native Usecase, not a Flowchart model conversion.
- Extraction copies internal edges once after the first member, matching Graphlib edges(node),
  which ignores its argument. Later source copies only overwrite identical labels; repeated scans
  are unnecessary. The original node insertion order is preserved.
- Compiled styles now share one safe, insertion-ordered map between measurement and paint. Label
  properties reach HTML div/span and SVG text with source !important precedence and color -> fill;
  shape properties stay on shapes. Notes inherit default class styles during measurement too.
- SVG roots expose the six Usecase font custom properties using the operation-owned root protocol.
  Deferred roots preserve properties while replacing only tracked viewport placeholders. Usecase
  does not impose a white background. Shared label emission accepts CSS without Mermaid entity
  decoding; existing unstyled wrappers preserve their behavior.
- SVG boundary labels honor subGraphTitleMargin.top. Dagre-only extraction helpers are independent
  of layout-elk; ELK-only finish/midpoint helpers are now feature-gated.

# Verification

Prior port-alignment checks:

- Adapter flat and SeparateChildren oracle test passed: run 7a19271a-3877-484e-b524-1eb2c86e4a08.
- Two Usecase ELK integration tests passed: run a42f9382-edf0-44f8-a0d0-3345a35f45a9.
- Lower importer constructor test passed: run bcd600fc-bb67-4061-9d94-283f377ae915.
- Temporary elkjs 0.9.3 oracle: target/mermaid-12-admission/usecase-port-alignment.cjs/.json.
  Root baseValue 40 target offsets default [15,20,25], Center [10,20,30]; SeparateChildren baseValue
  24 default [15.4,20,24.6], Center [10.8,20,29.2]. Root and child fixtures intentionally differ.

This checkpoint:

- `cargo nextest run -p merman-render --features layout-elk --test usecase_svg_test --lib -E
  'test(usecase) | test(svg::parity::root_svg) | test(svg::parity::label)'`: 35 passed,
  run 062c5912-8238-4109-9928-02f6799b6f17.
- After the extraction scan simplification, insertion-ordered style map and deferred-root property
  assertion, focused rerun with `-E 'test(usecase) | test(deferred_document_tracks_root_attributes)'`:
  17 passed, run 476ca3f4-ee3e-40a2-ae7d-e54d9715f4c4.
- `cargo check -p merman-render --no-default-features`: passed without warnings after gating the
  two ELK-only helpers. Scoped rustfmt and staged diff check passed.
- Temporary browser oracle target/mermaid-12-admission/usecase-styles-probe.cjs/.json uses the
  already verified Mermaid 12 runtime and Chrome headless shell. It confirms root properties,
  default/named/inline style precedence, SVG color -> fill and HTML color. It also confirms
  minNodeWidth affects SVG shapes through labelHelper.withMinWidth, not only HTML foreignObjects.
  Probe setup initially used invalid relationship label syntax and DOMRect.toJSON; corrected to
  `A link@-- action --> B` and explicit width/height. No oracle output was accepted as a golden.

# Remaining Work / Next Action

Usecase is still not admitted: core renderer metadata remains None. Do not promote its catalog or
bulk-refresh goldens from the current Merman SVG.

1. Dagre self-loops need source expansion. IMPORTANT correction to worker handoff: the source
   shouldMergeSelfLoopSegments allowlist in dagre/index.js excludes Usecase. Do NOT reuse the
   compact merged Flowchart loop. Source prepareLayoutForDagre creates two labelRect helper nodes
   and three named cyclic-special edges, then Usecase retains the segments. Study applyDagreLayoutResult
   and the actual runtime's accessibility/DOM output before extending the native prepared model.
   Current native Dugong self-loop route is still a source gap.
2. Extracted boundary SVG is still flattened; exact nested DOM and long-title/package/source numeric
   geometry comparison remain pending. Direction/external-connectivity tests establish behavior,
   not full layout or DOM admission. Child painted bounds currently include route polylines and
   label boxes; exact curve extrema/browser bbox differences need the same bounded policy as other
   families.
3. Host measurement and Markdown SVG remeasurement need a deliberate contract. Existing
   environment.rs builtin_operation_carrier only authorizes builtin routes; host callbacks remain
   observable. Do not blindly cache/reuse callbacks across preparation and emission. Shared Markdown
   helpers in text/metrics.rs parse/wrap/measure separately from SVG label emission; Kanban's
   prepared-label geometry is useful prior art, but not blanket host-reuse authorization.
4. Full shape/marker/style DOM, handDrawn behavior and remaining source geometry still need family
   comparisons. No newly claimed full parity. Independent parser/layout/render review remains open.
5. Ordered JSON recursion/resource policy and externally constructed model validation still need
   review; the prior reused-runtime disconnected JSON-table observation remains unresolved.
6. Continue remaining U9–U15 work: Agentflow, existing-family/theme deltas, editor/FFI/Web/Playground,
   feature/license closure, genuine upstream golden refresh, coherent bundle and version promotion.

No Cargo process is active at this checkpoint. Reuse target and serialize builds. Existing worker
/root/usecase_layout completed the boundary implementation and is available for follow-up; other
workers are idle/completed. Protect unrelated edits and use explicit UTF-8 Python file operations.

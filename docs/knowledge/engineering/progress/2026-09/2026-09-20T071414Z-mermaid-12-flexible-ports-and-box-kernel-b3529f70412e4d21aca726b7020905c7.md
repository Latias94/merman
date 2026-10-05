---
type: "Work Progress"
title: "Mermaid 12 flexible ports and Box kernel"
description: "Verified source NetworkSimplex port flexibility and EPL Box packing; dispatch and full admission remain open."
timestamp: 2026-09-20T07:14:14Z
record_id: "b3529f70412e4d21aca726b7020905c7"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "60965f255"
supersedes: "a75a923d906a4794b24cd68c3a98df29"
---

# Summary

The full U1-U15 Mermaid 12 goal remains active. This turn made concrete implementation progress
in two focused commits. Production reference/default promotion is still pending; the selected
baseline remains 11.17.2. No push, PR or release occurred. Prior runtime, signature and layering
receipts remain valid; do not regenerate them without changed inputs.

# Details

## NetworkSimplex and node-level flexibility

Commit `cb8d2c3e9` replaces the simplified fixed-port placer with the selected Java source's
auxiliary constraints: integral port/anchor preparation, corner/port vertices for flexible nodes,
layer separation, north/south and in-layer edge constraints, connected components, long-path
weights and two-path postprocessing. Path following and component walks use explicit loops.
The supported flexibility enum contains NONE and PORT_POSITION, the target-reachable values;
size-changing flexibility modes are not advertised.

`ElkInputNode` and `LNode` carry `node_flexibility` and the node's own optional
`ports_surrounding` property. The latter is used by source eligibility checks, while actual
port-chain spacing inherits the containing graph's spacing, as in ELK. Adapter group nodes get
PORT_POSITION; children do not inherit it. This addresses the prior focused review's P2.
Processor work admission counts extra corner and port vertices and rejects insufficient budgets
before mutation. This uses the existing processor-wide budget contract; it does not claim a new
per-simplex-iteration cancellation contract.

Evidence:

- Existing 392 kernel/adapter regressions passed after the rewrite:
  `b98874db-1b01-4a52-a46b-9dacc60caab1`.
- Actual elkjs 0.9.3 fixed/movable port case: five nodes, two incoming and two outgoing edges on
  the tall middle node. PortPosition gives four straight edges rather than two, without changing
  the node's 160px height. All five node y positions match after adding root padding. The test
  lives in `p4nodes/network_simplex.rs`; ignored collector and exact outputs are
  `target/mermaid-12-admission/flex-oracle.cjs` and `flex-{NONE,PORT_POSITION}.json`.
- Three additional tests passed for node/child scope, fixed/crowded eligibility and transactional
  work admission: `1ebd6b60-4bc9-4d44-a625-59fa63670713`.
- 31 affected Flowchart ELK renderer tests passed:
  `2dc30c65-ddca-4ddf-bf00-e1e2f84d37b8`.
- The prior reviewer has not independently reviewed this new implementation. Final code review
  remains open; do not describe the old plan review or P2 identification as implementation approval.

## Box packing

Subagent `/root/elk_packing` implemented Box; root integrated and validated it. Commit `60965f255`
adds `merman_elk_layered::algorithms::box_layout` in the existing EPL crate. Reusing this boundary
avoids another crate/license/work-control layer. The measured-rectangle API returns positions,
extents and content shifts in input order. It ports stable priorities, interactive order, sample
variance row width, padding/minimum size and expansion. Only Mermaid-reachable SIMPLE is exposed;
no grouped packing modes, edge routing or random behavior are claimed.

Eleven tests compare actual elkjs 0.9.3 geometry and cover cancellation and finite-input errors.
The source behavior that only each row's last expanded box shifts its content is preserved.
Together with the final port geometry assertion, all 12 targeted tests passed:
`46ff1e23-fdae-4f21-b967-bc279dc6e3d0`.

README records source and scope. The canonical Eclipse ELK component now covers `plugins`,
including common/core utilities and Box, instead of naming only the layered directory. Ran:

- `python scripts/verify-third-party-licenses.py --write`
- `python scripts/sync-release-legal-materials.py --write`

Both succeeded; generated notice copies changed only the two source/description lines. No
workspace license, dependency or feature default changed. Box is NOT yet admitted through
renderer dispatch or mixed-container integration.

# Next Action

1. Continue U3 layout resolution and measured graph dispatch, then U5/U8 integration. Current
   `merman-render/src/lib.rs::uses_elk_layout` checks only literal `layout == elk`; missing ELK
   features still raise MissingCapability in Flowchart/Class and ER. Target
   `rendering-util/render.ts::getRegisteredLayoutAlgorithm` chooses registered requested layout,
   then family fallback, then Dagre; execution failures must never trigger fallback. Ensure SVG
   consumers use the same resolved identity, not the raw request. Class still constructs a
   Dugong graph before its ELK path; inspect and refactor measurement ownership rather than
   duplicating it. State/Requirement ELK paths remain to implement.
2. U5 postprocessing remains open. Target `elk/render.ts:1378` evenGroupFrames must preserve the
   original edge-owner coordinate origin while moving frames. `straightenTerminalJogs` and
   `straightenEdgeTerminals` at 1596/1704 run AFTER clipping: they keep both endpoints, move an
   entire channel run, use source constants (16 jog, 30 terminal run, .01 epsilon), and accept a
   candidate only when total strict crossings do not increase. Do not run this prematurely on
   raw kernel routes or confuse it with the existing marker-stub shortening.
3. Add remaining source algorithms (stress, force, mrtree, radial, rectpacking, sporeOverlap) in
   the same EPL boundary, then actual root/container dispatch. Existing Box is ready for that
   integration. Root versus container algorithm allowlists and crossing-edge cleanup remain
   required. No extra algorithm worker is currently running; elk_packing completed without
   running Cargo or committing.
4. U1 complete companion selection/receipt, U2 target defaults/themes and temporary Swimlane
   bridge removal, U9-U13 families/Agentflow/Usecase/editors/bindings, U14 feature/product defaults,
   and U15 final coherent reference admission/size/integration remain open. Preserve full scope.
5. Cargo stays serial; reuse target. No build session remains running at this checkpoint.

# Citations

- [Accepted plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Prior container checkpoint](2026-09-20T065510Z-mermaid-12-container-option-boundaries-a75a923d906a4794b24cd68c3a98df29.md)

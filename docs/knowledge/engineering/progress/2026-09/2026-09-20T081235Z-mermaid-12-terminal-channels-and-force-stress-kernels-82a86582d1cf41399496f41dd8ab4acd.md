---
type: "Work Progress"
title: "Mermaid 12 terminal channels and Force Stress kernels"
description: "Clipped route postprocessing, source force and stress algorithms, numerical review fixes and EPL projections."
timestamp: 2026-09-20T08:12:35Z
record_id: "82a86582d1cf41399496f41dd8ab4acd"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "878dcd859"
supersedes: "307284ebe70144e49f4577305fc83183"
---

# Summary

The full Mermaid 12 U1-U15 / R1-R12 goal remains active and far from finished. Production
reference/default projections still select 11.17.2; the genuine Mermaid 12 reference is staged.
No push, PR, or release occurred. Working tree was clean after the implementation commits.

# Completed work

- `044623db7` splits Flowchart edge geometry into clipping and finalization. ELK terminal
  straightening now runs between them across all routes in source edge order. It moves the
  entire qualifying channel, preserves ports, rejects additional strict crossings, honors
  `elk.straightenEdges: false`, and charges route/segment work. Viewbox, SVG paths and data-points
  use the same postprocessed cache. ELK compound edges use root coordinates consistently with
  emission. Diagnostic recomputation does not replace the cache. Dagre retains its existing
  processing, including the original raw-point fallback for degenerate clipped routes.
- `878dcd859` adds EPL Force and Stress kernels. Force implements source Fruchterman-Reingold
  and Eades models, Java randomness, sparse adjacency, component DFS order (including source
  cycle-edge multiplicity), label particles, packing and rectangle clipping. Stress uses the
  actual Force initializer, shortest-path weights, sequential localized optimization, fixed
  nodes, axis selection and per-edge desired lengths. Both share the real importer/exporter;
  no second approximate graph pipeline was added.
- Independent Force review found nonterminating jitter at coincident 1e20 coordinates and
  omitted repeated edge/label work. Both are fixed with NumericStagnation and complete iteration
  charging. Tests cover both failures. Stress admits quadratic state before allocating it.
- README and canonical third-party notice now include Force/Stress translations. Existing
  generators refreshed the actual distribution projections. No feature or dependency was added.
- `0ee9685fd` updates a pre-existing parallel-edge-label regression for Mermaid 12 container
  geometry. The test now explicitly selects NONE alignment, preserving identity/route assertions.
  Actual elkjs 0.9.3 with identical measured nodes (75.84 x 54), labels (13.44 x 24), and target
  root/container options yields global label top-left y=117.1/72.1, hence centers 129.1/84.1.
  The signed fixture itself was not modified. Temporary oracle:
  `target/mermaid-12-admission/thickness-measured.cjs` and corresponding JSON.

# Verification

- Force/Stress: `cargo nextest run -p merman-elk-layered -E 'test(algorithms::force) | test(algorithms::stress)'`:
  11 passed; run `eceff8ef-f8fe-4468-bf08-a8e79d4b12f7`. Numerical oracle tables cover real elkjs
  0.9.3 chains, cycles, components, interactive inputs, fixed nodes, axes, labels and endpoints.
- Flowchart SVG/ELK scope: `cargo nextest run -p merman-render --features layout-elk -E
  'binary(flowchart_svg_test) | (test(flowchart) & (test(terminal) | test(elk)) & not binary(flowchart_layout_test))'`:
  128 passed; run `d51803ff-60c0-45b9-943a-341ba632c82d`.
- The broader initial scope exposed the old label-coordinate assertions (78 passed, 1 failed,
  remaining tests cancelled). They were investigated against actual elkjs, not normalized away.
  After the focused correction, both ELK layout tests passed with `--test flowchart_layout_test
  -E 'test(elk)'`, run `71305680-e295-41ff-b2a2-7b15e73a653d`.
- Terminal module covers eight pinned geometry cases, reflected orientations, crossing rejection,
  endpoint contacts and budget rejection. A controlled positioned-graph SVG test proves that
  five points become three while ports stay fixed and trace-enabled SVG remains identical.
  An initial fan-graph smoke did not produce a qualifying jog and was replaced, not weakened.
- `cargo fmt --all -- --check`, `git diff --check`, third-party license verifier and
  `python scripts/sync-release-legal-materials.py --check` passed (382 projected files).
- Cargo ran serially against the normal target. No unchanged whole-workspace checks were repeated.

# Remaining work and next action

Force/Stress/Box are measured flat-graph kernels only. Root/container dispatch, hierarchy
scheduling, source node micro-layout, and effective option/label defaults remain integration
work. Do not advertise these as working Mermaid layout names yet. Stress's effective label
inline default is true; its caller must supply it. The resolved seed parameter is not a license
to bypass operation-owned randomSeed=0 resolution.

Next: complete the other reachable kernels (mrtree, radial containers, rectpacking,
sporeOverlap), then wire typed root/container dispatch and family projections. U5 also still
needs frame equalization, degenerate-node anchor alignment, marker terminal minimums and ELK
line hops; this commit implements terminal-channel straightening only. State/Requirement and
Mindmap layout paths, Agentflow/Usecase, default configuration/appearance promotion, other
family deltas, editor/binding/feature closure, final reference selection/admission, size checks
and final independent review remain as recorded by the full plan and predecessor checkpoint.

All selected source revisions and staged reference identities are unchanged from predecessor
`307284ebe70144e49f4577305fc83183`. Do not promote the partially staged target bundle as-is or
relabel old fixtures. No active subagent or Cargo process remains at this checkpoint.

Memory validation passed with existing legacy warnings. Rollup rendering declined to replace
the unadopted legacy current-state/log files; they remain untouched. This immutable shard is
the continuity record, so no rollup migration is needed for this implementation task.

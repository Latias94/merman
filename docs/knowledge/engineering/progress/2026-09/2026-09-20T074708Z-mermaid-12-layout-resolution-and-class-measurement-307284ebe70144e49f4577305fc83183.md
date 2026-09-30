---
type: "Work Progress"
title: "Mermaid 12 layout resolution and Class measurement"
description: "Registered-layout fallback across renderer and bindings, backend-neutral Class measurements, and reviewed simplex coordinate fixes."
timestamp: 2026-09-20T07:47:08Z
record_id: "307284ebe70144e49f4577305fc83183"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "d43862c79"
supersedes: "b3529f70412e4d21aca726b7020905c7"
---

# Summary

The complete U1-U15 / R1-R12 goal remains active and far from finished. This checkpoint adds
three focused implementation commits. The production bundle/defaults still select 11.17.2;
Mermaid 12 runtime and generated defaults remain staged. No push, PR, or release occurred.

# Details

## Registered graph layout selection

`d43862c79` introduces a small typed selector shared by Flowchart, Class, and ER. Literal `elk`
selects ELK only when compiled; an absent or unknown loader falls back to Dagre before capability
admission, matching target `rendering-util/render.ts::getRegisteredLayoutAlgorithm`. Case is
significant. Compiled ELK denied by host policy still raises MissingCapability. Selected-backend
errors, budgets, cancellation and missing math do not trigger fallback.

Flowchart edge clipping and curve selection now use the actual layout's ELK flag instead of raw
config or syntax alias. Removed no-feature error stubs and duplicate ER backend enum/setting.
Prepared artifact metadata retains the original request. The selector only covers the existing
Dagre/ELK graph adapters; it does NOT claim all target algorithms or families are registered yet.

The modern profile now supplies `flowchart.layout: elk`, replacing obsolete defaultRenderer.
Profile aspect status describes the requested presentation independently from operation admission:
a lean build can report the ELK aspect blocked while the fallback operation is ready. Removed a
binding projection assertion incorrectly coupling those states. Updated actual facade/binding
coverage and user docs/ADR 0077. Missing required capabilities remain a subset of required IDs.
Old duplicate rejection tests were replaced by a compact end-to-end matrix comparing actual
geometry and SVG with explicit Dagre, including legacy syntax and denied-host-policy coverage.

## Class measurements

`0beea41db` removes the Dugong graph as Class ELK input. Shared family measurements own stable
node/edge identity, dimensions, namespace labels and typed terminal-label metrics. Dagre alone
converts terminal metrics to extras; ELK directly projects measured inputs. Removed capture-engine
and result enums plus optional operation-work state. The hidden Dagre debug helper reuses the
measurement and projection. Namespace titles are measured once; ELK requires an operation seed.

Subagent class_measurement implemented only class.rs and class/measured.rs; root reviewed and
validated. A regression covers forward parent references: node insertion order and parent mutation
order are distinct and must both be retained to preserve namespace sibling order.

## Independent review and simplex correction

`c85e13840` fixes both findings from independent review_flexible_ports of `cb8d2c3e9`:

- Flexible head/tail height constraints exposed an unused dense filling allocation proportional
  to coordinate span. Non-balancing normalization now translates coordinates without that array.
  Coordinate arithmetic uses i64 intermediates and checked i32 storage. Out-of-range coordinates
  propagate NetworkSimplexError through PipelineError, rather than saturating or overflowing.
- All source V_BOTTOM outside label placements now receive size_delta, including fractional
  heights. Tests cover 160.5 and 100,000,000.5 heights, every bottom label position, i32::MAX
  non-balancing coordinates, cumulative overflow and the public pipeline error path.

The worker fixed its review findings; root inspected the diff and ran tests. This is not a claim
of a final independent review of the whole goal. Existing processor-wide work admission remains;
no new claim of cancellation during every simplex iteration is made.

# Verification

Cargo stayed serial and used the existing target. All workers avoided Cargo.

- Kernel + adapter: 411 tests passed, run `807f87e7-7b83-41a3-a1af-6a08adafc6f0`.
- ELK-enabled renderer/facade/bindings Class, layout resolution, presentation, SVG plan and typed
  family API: 193 passed, run `cf3e2681-790e-4855-80df-a522a21c3a05`.
- Lean same affected scope: 181/182 passed in `b196792f-b53d-4410-9821-08b95260c268`;
  the only failure was an old renderer-aware Class detection expectation from before the core
  Mermaid 12 detector change. Updated against target classDetector-V2.ts, then that exact test
  passed in `887975bf-84ff-482f-823c-90fb0f4c3547`. It also passed in the ELK-enabled 193-test run.
- Initial focused lean fallback/config tests: 7 passed, `9c27f61a-dfea-4bb4-8a00-6c57cf655a87`.
- `cargo fmt -p merman-render -p merman-bindings-core -p merman -p merman-elk-layered --check`
  passed. `git diff --check` passed.

No need to rerun these unchanged inputs. Final size, feature defaults, legal closure and full
release admission remain later work, not established by these scoped checks.

# Next Action

1. Continue U3: State/Requirement ELK, Mindmap's source fallback chain, all actual root/container
   algorithm identities and family dispatch. The new selector intentionally only knows Dagre/Elk
   until the additional algorithms work. Do not claim fallback is an implementation of them.
2. U5 source postprocessing remains unimplemented: evenGroupFrames preserves edge owner origins;
   straightenTerminalJogs/straightenEdgeTerminals run on clipped routes, move whole channel runs
   while preserving ports, and reject increased strict crossing counts. Source constants are 16,
   30 and .01. Flowchart computes clipped paths per edge in edge_geom/compute.rs, then caches them
   in viewbox.rs; any graph-wide postprocessing must happen after every clipped route exists and
   before curves/markers/viewbox are finalized. Existing Swimlane line-hop engine can be examined
   for genuine shared semantics, not copied wholesale or enabled blindly.
3. U6-U8: Box kernel exists but is not integrated. Stress, force, mrtree, radial, rectpacking and
   sporeOverlap remain to port inside the EPL crate. Stress source first runs ForceLayoutProvider
   for non-interactive initial positions: do not implement stress using unrelated initialization.
   Source files are under repo-ref/elk/plugins/org.eclipse.elk.alg.force. No algorithm worker is
   currently running.
4. U1 complete companion graph/receipt, U2 default/theme promotion and temporary Swimlane bridge
   removal, U9-U13 family changes and complete Agentflow/Usecase/editor/binding support, U14
   product defaults, U15 coherent fixture/provenance/size/integration remain open. Preserve the
   accepted full scope. Prior staged reference, package signatures and browser receipts remain
   valid; do not regenerate unchanged evidence. Do not label old baselines 12.0.0.
5. All workers completed, no Cargo process remains. Changes are in focused commits. Final
   independent implementation review remains required; plan review does not replace it.

# Citations

- [Accepted plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Prior checkpoint](2026-09-20T071414Z-mermaid-12-flexible-ports-and-box-kernel-b3529f70412e4d21aca726b7020905c7.md)

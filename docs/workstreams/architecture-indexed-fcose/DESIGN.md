# Architecture Indexed FCoSE

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

Architecture diagrams are the largest remaining standard-canary performance gap after the completed
fearless refactor lane. The current render path is already typed, but Architecture layout still pays
for a string-keyed `manatee::Graph` adapter before FCoSE converts the same graph back into indexed
internal state.

This lane removes that accidental boundary and makes FCoSE expose the same kind of indexed layout
entry point that COSE-Bilkent already provides.

## Relevant Authority

- Existing workstreams:
  - `docs/workstreams/fearless-refactor/STATUS.md`
  - `docs/workstreams/fearless-refactor/COMPLETION_AUDIT.md`
- Performance docs:
  - `docs/performance/PERF_MILESTONES.md`
  - `docs/performance/COMPARISON.md`
  - `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md`
- Source boundaries:
  - `crates/merman-render/src/architecture.rs`
  - `crates/manatee/src/algo/fcose/mod.rs`
  - `crates/manatee/src/algo/cose_bilkent/mod.rs`
  - `crates/manatee/src/graph/mod.rs`

## Problem

The Architecture layout path constructs string-owned FCoSE input and receives string-keyed output
even though the hot FCoSE simulation runs on indices internally. This adds avoidable allocation,
hashing, map construction, and adapter code to the slowest remaining canary.

The prior performance snapshot showed:

- `architecture_medium` layout ratio: about `9.44x` slower than mmdr.
- `architecture_medium` end-to-end ratio: about `4.23x` slower than mmdr.
- Mindmap already uses an indexed COSE-Bilkent path, which proves this boundary shape fits the
  local graph-layout architecture.

## Target State

- FCoSE has an indexed input/output API alongside the existing string-keyed compatibility API.
- The existing public `manatee::Graph` path remains as a compatibility adapter.
- Architecture layout builds indexed FCoSE input directly from its typed model.
- Architecture layout no longer builds a transient string-keyed `manatee::Graph` just to call FCoSE.
- Architecture parity gates remain green, root overrides do not grow, and layout-stage performance
  improves or the attempted shape is rejected with evidence.

## In Scope

- Add indexed FCoSE data types and `layout_indexed` entry point.
- Refactor FCoSE adapter construction so the string-keyed path delegates to indexed internals.
- Wire Architecture layout to the indexed FCoSE path.
- Delete redundant Architecture-side string graph construction when the indexed path covers it.
- Add or adjust tests that compare indexed and compatibility FCoSE behavior.
- Record fresh performance and parity evidence.

## Out Of Scope

- Rewriting the FCoSE algorithm itself.
- Changing Mermaid parity semantics or adding new fixture-derived overrides.
- Reworking typed render dispatch.
- Reworking text measurement caches.
- Changing public `merman` APIs.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| FCoSE adapter overhead is meaningful for Architecture layout. | Medium | Architecture layout is the largest stage gap; source shows string graph construction before indexed internals. | If algorithmic simulation dominates completely, the lane may close after adding indexed API with limited Architecture win. |
| Indexed FCoSE can mirror COSE-Bilkent's boundary without broad API breakage. | High | `cose_bilkent::layout_indexed` already exists and Mindmap uses it. | If FCoSE constraints need richer IDs, keep a small local mapping instead of leaking strings through the hot path. |
| Architecture parity should remain stable under deterministic node order. | Medium | The current string-keyed path already has deterministic ordering inputs. | If ordering drifts, add explicit stable index construction before replacing the compatibility path. |

## Architecture Direction

The target boundary is:

1. `manatee::algo::fcose` owns indexed layout input and output types.
2. The existing `manatee::Graph` API becomes an adapter that maps strings to indices once.
3. `merman-render::architecture` builds indexed nodes, edges, compounds, and constraints from its
   typed model directly.
4. FCoSE internals consume indices and return `Vec<Point>`-style positions, not `BTreeMap<String, Point>`.

This deepens the graph-layout module: string identity remains at public compatibility boundaries,
while simulation and diagram-specific hot paths use compact typed indices.

## Closeout Condition

This lane can close when:

- Architecture uses indexed FCoSE without a transient string-keyed graph adapter.
- Existing FCoSE compatibility callers still work.
- Targeted tests and Architecture parity gates pass.
- Performance evidence shows a useful improvement, or documents why the indexed boundary was not
  enough and what the next hotspot is.
- Follow-on refactors are split into separate workstreams instead of silently expanding this lane.

Closeout status: satisfied on 2026-05-28. Follow-on candidates are typed render dispatch
consolidation and text measurement cache/context; both are outside this lane.

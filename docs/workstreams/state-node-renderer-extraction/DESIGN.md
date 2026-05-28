# State Node Renderer Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

After edge rendering moved to `state/edge.rs`, `state/render.rs` still owns a large leaf-node
renderer with shape-specific markup, style parsing, label measurement, label HTML, links, and
Rough.js cache access. Extracting this node emitter is the next durable boundary in the state SVG
renderer split.

## Relevant Authority

- Existing docs:
  - `docs/rendering/REFACTOR_TODO.md`
  - `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
- Related workstreams:
  - `docs/workstreams/state-edge-renderer-extraction/`

## Problem

`render_state_node_svg` is a dense node-local renderer embedded in the root orchestration module.
That makes root traversal harder to audit and makes future state shape work risky because
orchestration and shape emission are edited together.

## Target State

`state/render.rs` keeps root traversal, clusters, edge group ordering, and nested root recursion.
`state/node.rs` owns leaf-node SVG emission, including node-local style parsing, Rough.js path
caching, label measurement, label HTML, and link wrapping.

## In Scope

- Extract `render_state_node_svg` and its node-local helpers to
  `crates/merman-render/src/svg/parity/state/node.rs`.
- Wire `state/mod.rs` and `state/render.rs` to call the extracted module.
- Preserve current SVG output behavior and timing counters.
- Record fresh validation evidence.

## Out Of Scope

- Rewriting node shape emission.
- Splitting each state shape into separate files.
- Changing root viewport derivation or generated overrides.
- Performance benchmarking.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `render_state_node_svg` is node-local enough to move as a unit. | High | It begins after edge extraction and extends to the end of `render.rs`. | May require exporting a small helper or keeping a tiny wrapper in `render.rs`. |
| Existing state gates cover output-sensitive node shapes. | Medium | State fixtures cover start/end, notes, composites, class styles, and labels. | Add focused test before closeout if compare-state misses a path. |
| Keeping `StateRenderCtx` shared is the right first boundary. | High | Edge extraction used the same pattern successfully. | A follow-up can introduce a narrower node context. |

## Architecture Direction

Preserve the shared `StateRenderCtx` for now and move implementation ownership, not data ownership.
This keeps the refactor behavior-preserving while making `render.rs` a cleaner orchestration module.

## Closeout Condition

This lane is complete. State node rendering now compiles from `state/node.rs`, validation gates
passed, and follow-on root/cluster splits are deferred to later bounded lanes.

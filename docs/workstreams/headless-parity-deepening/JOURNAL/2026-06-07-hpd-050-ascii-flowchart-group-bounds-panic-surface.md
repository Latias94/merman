# HPD-050 - ASCII Flowchart Group Bounds Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the COSE-Bilkent radial tree hardening, the next public renderer audit checked non-SVG
surfaces that still consume tree-shaped Flowchart models. The SVG Flowchart path already had
explicit-stack coverage for deep `subgraph` chains, but ASCII rendering still computed raw group
bounds through recursive child-group calls before applying terminal layout offsets.

That left a release-boundary risk: a `flowchart TB` model that was valid for parsing and SVG layout
could still depend on the Rust call stack when rendered through the public ASCII API.

## Changes

- Replaced recursive ASCII group raw-bounds calculation with explicit heap-backed enter/exit
  frames.
- Built node-layout and group-index lookup tables once per bounds query so child groups can be
  resolved from completed postorder state.
- Preserved the existing aggregation semantics:
  - direct node bounds still win when a member id names a laid-out node;
  - child group bounds are included only after the child group is complete;
  - self-references and cycles do not recursively re-enter the same group;
  - title wrapping, title space, and group padding remain unchanged.
- Added a public `merman` ASCII API regression that renders a `512`-level Flowchart subgraph chain
  on a `64KB` stack.

## Verification

- `cargo +1.95 nextest run -p merman --features ascii --test ascii_api render_ascii_model_handles_deep_flowchart_subgraph_chain_with_small_stack` -
  passed, `1` test run.
- `cargo +1.95 nextest run -p merman --features ascii --test ascii_api` - passed, `7` tests run.

## Boundary

No SVG baseline, root viewport formula, Mermaid parity fixture, graph spacing constant, or renderer
theme behavior changed. This slice only hardens ASCII group-bounds traversal against deep accepted
Flowchart subgraph chains.

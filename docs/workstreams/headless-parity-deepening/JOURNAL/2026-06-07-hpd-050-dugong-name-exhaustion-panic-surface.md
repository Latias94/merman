# HPD-050 - Dugong Name Exhaustion Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`dugong` had two production `unreachable!()` fallbacks in layout-internal name allocation loops:

- `acyclic::unique_rev_name(...)` scanned `rev1`, `rev2`, ... for a free reversed-edge name;
- `normalize::add_dummy_node(...)` scanned `_d`, `_d1`, ... for a free dummy node id.

Normal graphs will find a free name long before `usize` is exhausted, so these were not practical
user-input crashes. They were still library-code panic surfaces in the release boundary audit and
could be made defensive without changing ordinary Dagre behavior.

## Changes

- Changed `unique_rev_name(...)` to return `Option<String>` and skip inserting the affected
  reversed feedback edge if the theoretical name space is exhausted.
- Changed `add_dummy_node(...)` to return `Option<String>` and stop normalizing the current long
  edge if the theoretical dummy-node name space is exhausted.
- Preserved normal `rev*` edge naming, `_d*` dummy node naming, cycle breaking, long-edge
  normalization, and undo behavior.

## Verification

- `cargo +1.95 fmt -p dugong` - passed.
- `cargo +1.95 nextest run -p dugong acyclic normalize` - passed, `23` tests run.
- `rg -n 'unreachable!|panic!|expect\(|unwrap\(' crates/dugong/src/acyclic.rs crates/dugong/src/normalize.rs` -
  no matches.
- `git diff --check` - passed.

## Boundary

This is a Dugong layout-engine panic-surface cleanup only. It does not change rank assignment,
feedback-arc selection, greedy/DFS cycle-breaking order, normal reversed-edge naming, long-edge
dummy chain semantics, edge-label dummy behavior, layout geometry, Graphlib APIs, SVG baselines,
root viewport formulas, or Mermaid parity residual classification.

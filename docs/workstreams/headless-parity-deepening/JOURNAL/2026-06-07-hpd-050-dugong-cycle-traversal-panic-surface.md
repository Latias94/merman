# HPD-050 - Dugong And Graphlib Cycle Traversal Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the Architecture `iconText` XHTML cleanup, the next audit pass looked for remaining
production `panic` / `unwrap` / recursion candidates that could be reached through public graph or
layout APIs. The strongest adjacent candidates were in the Dugong/Graphlib cycle traversal front:

- Graphlib `find_cycles(...)` still used recursive Tarjan `strongconnect(...)`;
- Dugong's default Dagre `acyclic::run(...)` path still used recursive DFS feedback-arc traversal.

Both paths can be exercised by arbitrary public graph inputs, and `acyclic::run(...)` is part of
the Dagre layout pipeline used by Mermaid-facing renderers.

## Red Signal

Focused small-stack regressions reproduced stack overflow before the fixes:

- `find_cycles_handles_deep_successor_chains_with_small_stack` overflowed on a `2,048`-edge
  public Graphlib successor chain on a `64KB` stack;
- `acyclic_run_handles_deep_dfs_chains_with_small_stack` overflowed on a `2,048`-edge Dugong DFS
  acyclicer chain on a `64KB` stack.

The Graphlib red case was acyclic, proving the failure came from traversal depth rather than cycle
complexity. The Dugong red case used the default DFS acyclicer path, matching Dagre's ordinary
cycle-removal route when `acyclicer` is absent, `"dfs"`, or unknown.

## Changes

- Replaced Graphlib recursive Tarjan traversal with explicit heap-backed frames while preserving:
  - successor iteration order;
  - index / lowlink propagation;
  - SCC stack behavior;
  - existing self-loop cycle filtering in `find_cycles(...)`.
- Replaced Dugong recursive DFS feedback-arc traversal with explicit heap-backed frames while
  preserving:
  - Dagre node insertion order;
  - out-edge iteration order;
  - self-loop skip behavior;
  - back-edge feedback-arc collection.
- Added focused regressions for both public deep successor-chain paths.

## Verification

- `cargo nextest run -p dugong-graphlib find_cycles_handles_deep_successor_chains_with_small_stack` -
  failed before the fix with stack overflow; passed after iterative Tarjan traversal.
- `cargo nextest run -p dugong acyclic_run_handles_deep_dfs_chains_with_small_stack` - failed
  before the fix with stack overflow; passed after iterative DFS feedback-arc traversal.
- `cargo nextest run -p dugong-graphlib --test alg_test` - passed, `23` tests run.
- `cargo nextest run -p dugong --test acyclic_test --test greedy_fas_test` - passed, `15` tests
  run.
- `cargo nextest run -p dugong-graphlib` - passed, `99` tests run.
- `cargo nextest run -p dugong` - passed, `278` tests run.
- `cargo nextest run -p merman-render --test class_svg_test` - passed, `26` tests run.
- `cargo nextest run -p merman-render --test flowchart_svg_test` - passed, `34` tests run.
- `cargo nextest run -p merman-render state` - passed, `17` tests run.
- `cargo fmt --check -p dugong -p dugong-graphlib` - passed.
- `git diff --check` - passed.

## Boundary

No SVG baseline, root override, Mermaid parity fixture, Architecture formula, or rendered output
formula changed. This slice is stack-safety hardening for public Graphlib cycle detection and
Dugong's default Dagre cycle-removal traversal, not a parity-root residual fix.

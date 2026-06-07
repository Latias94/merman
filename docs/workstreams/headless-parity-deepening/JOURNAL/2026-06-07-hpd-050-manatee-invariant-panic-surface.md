# HPD-050 - Manatee Invariant Panic Surface

Date: 2026-06-07

## Context

Recent HPD-050 slices hardened manatee's deep FCoSE compound traversal and COSE-Bilkent radial tree
placement. The remaining target in this slice was narrower: internal invariant panics that do not
need to crash the layout library if future graph construction or relative-placement data drifts.

This slice intentionally does not tune solver formulas, force constants, seeded randomness,
root-bounds calculations, or any Architecture/Mindmap residual.

## Change

- COSE-Bilkent `SimGraph::from_graph(...)` now skips an edge if its validated source or target id is
  unexpectedly missing from the node index instead of panicking on `expect("validated")`.
- FCoSE relative-placement component grouping now uses `Option` matching after the existing empty
  set guard instead of unwrapping the first set member.
- The COSE-Bilkent horizontal y-force diagnostic `panic!` remains visible as a separate solver
  invariant triage item.

## Verification

- `cargo +1.95 fmt -p manatee`
- `cargo +1.95 nextest run -p manatee`
- `rg -n 'expect\("validated"\)|set\.iter\(\)\.next\(\)\.unwrap\(\)' crates/manatee/src/algo/cose_bilkent/mod.rs crates/manatee/src/algo/fcose/mod.rs`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check
- `git diff --check`

## Notes

- No renderer SVG output, fixture baseline, root-bounds formula, Graphlib/Dagre behavior, or
  Architecture/Class residual classification changed.
- Existing manatee tests cover the normal COSE-Bilkent and FCoSE paths after this guard cleanup.

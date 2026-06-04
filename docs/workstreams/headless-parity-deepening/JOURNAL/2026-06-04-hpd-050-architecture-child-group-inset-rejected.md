# HPD-050 - Architecture Child Group Inset Experiment Rejected

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The nested aggregate edge report made `nested_groups_002/platform` look like a child-group boundary
problem: child group `data` owns both horizontal edges and accounts for aggregate `edge dw=-0.5`.
The production renderer already has a nested-group-specific phase in `GroupRectComputer`: child
group bounds are inset by `1.0px` on each edge before parent content is padded.

That made global inset tuning a tempting production path, so it needed a family-level experiment
rather than another local inference.

## Experiment

Temporarily changed:

```rust
let child_group_inset = 1.0;
```

to:

```rust
let child_group_inset = 0.75;
```

Then ran:

```powershell
cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_child_inset_075_hpd050.md
```

## Result

The experiment failed the family-level gate:

- Architecture `parity-root` expanded from the current `24` mismatches to `44`.
- `nested_groups_002` worsened from `+2.500` to `+2.750`.
- `group_port_edges_017` regressed back into the root queue at `+0.250`.
- Deep nested group rows regressed, including `deep_group_chain_027` and
  `batch6_deep_group_chain_crosslinks_094`.

The production code was restored to `child_group_inset = 1.0`, and
`git diff -- crates/merman-render/src/svg/parity/architecture/geometry.rs` is clean.

## Residual Boundary

Do not use global child-group inset tuning as the `nested_groups_002` fix. The evidence points at
child-group aggregate boundary drift, but the current global inset is protecting deeper nested rows
and previously closed rows. A future production fix needs a narrower source-backed phase model.

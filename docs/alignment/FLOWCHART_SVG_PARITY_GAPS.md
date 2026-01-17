# Flowchart SVG Parity Gaps (Mermaid@11.12.2)

This note tracks known remaining `compare-flowchart-svgs` DOM parity mismatches for the Stage B
flowchart renderer.

Reproduce:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Known mismatches

### `fixtures/flowchart/upstream_flowchart_v2_edges_in_and_out_subgraphs_spec.mmd`

Edge path command count (`C`) differs from upstream:

- `L_nat_internet_0` (upstream `C=2`, local `C=3`)
- `L_router_subnet2_0` (upstream `C=6`, local `C=2`)
- `L_subnet1_nat_0` (upstream `C=6`, local `C=2`)

### `fixtures/flowchart/upstream_flowchart_v2_self_loops_spec.mmd`

Self-loop special edges still diverge in curve segment count:

- `*-cyclic-special-2` (upstream `C=4`, local `C=2`): `A`, `B1`, `C2`, `D1`
- `*-cyclic-special-1` (upstream `C=4`, local `C=2`): `C1`
- `*-cyclic-special-mid` (upstream `C=2`, local `C=4`): `A`, `B1`, `C1`, `C2`, `D1`

## Next steps

- Move these from renderer-side heuristics into a layout-level fix by matching Dagre/Mermaid edge
  point generation (dummy chain placement + cluster interactions), so the SVG `d` command
  sequences match without post-processing.

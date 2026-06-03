# HPD-050 - Architecture Group Port Relocation And Repulsion

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous `group_port_edges_017` audit showed that the local outer group height matched
browser `bbAfterSegments.h=444.603px`, while the stored upstream SVG group rect used the later
final compound bbox height `462.448px`. A focused local experiment also showed that
`MANATEE_FCOSE_DISABLE_RELOCATE=1` made the row's group/root size exact, leaving only a uniform
translation. This pass checked whether relocation itself was the global fix or only a diagnostic
signal.

## Outcome

- Enhanced `tools/debug/arch_fcose_browser_probe_fixture_025.js` so the browser probe records
  `relocateComponent(...)`, first-iteration displacement stages, and compound CoSE nodes.
- Enhanced the xtask probe Markdown to expose a dedicated relocation table.
- Ran full Architecture `parity-root` with `MANATEE_FCOSE_DISABLE_RELOCATE=1`. It is not a global
  production fix: `group_port_edges_017` became exact, but the mismatch count rose from `25` to
  `27`.
- Re-ran the focused browser probe for `stress_architecture_group_port_edges_017` with Edge:
  `target\compare\architecture-fcose-probe-group-port-relocate-hpd050\stress_architecture_group_port_edges_017.fcose-browser-probe.md`.
- Compared that against local `MANATEE_FCOSE_DEBUG_RELOCATE`,
  `MANATEE_FCOSE_DEBUG_POSITIONS_ALL`, and `MANATEE_FCOSE_DEBUG_FORCES` runs.

## Findings

- First-run relocation matches local exactly:
  `orig=(0.000,8.500)`, current rect center `(26.799,22.441)`, delta
  `(-26.799,-13.941)`.
- Second-run original center also matches local:
  `orig=(1.500,17.750)`.
- The divergence therefore is not a wrong second-run `eles.boundingBox()` original-center input.
- The divergence starts in the second run's first CoSE tick, before constraint relaxation:
  - upstream `inner` compound: `repulsion=(0,250)`, displacement `(0,30)`;
  - local `inner` compound: `repulsion=(40,40)`, displacement `(6,6)`.
- The upstream branch is consistent with `layout-base` `IGeometry.getIntersection(...)` taking the
  non-overlap / near-vertical minimum-distance path for an almost-touching `inner`/`out1` pair
  after `ConstraintHandler.handleConstraints(...)`.
- Local currently lands on the touching/overlap branch for the same semantic phase, preserving the
  same compact vertical spread seen in `bbAfterSegments`.

## Verification

- `MANATEE_FCOSE_DISABLE_RELOCATE=1 cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_disable_relocate.md` -
  expected failure, `27` mismatches.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-fcose-probe-group-port-relocate-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed.
- `MANATEE_FCOSE_DEBUG_RELOCATE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-relocate` -
  passed.
- `MANATEE_FCOSE_DISABLE_RELOCATE=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-disable-relocate` -
  passed.
- `MANATEE_FCOSE_DEBUG_POSITIONS=1 MANATEE_FCOSE_DEBUG_POSITIONS_ALL=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-positions-all` -
  passed.
- `MANATEE_FCOSE_DEBUG_FORCES=1 MANATEE_FCOSE_DEBUG_EDGE_FORCES=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-forces` -
  passed.
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.

## Boundary

Do not fix this row by disabling relocation globally, changing group padding, exporting
layout-base compound rects directly, or globally changing `rects_intersect(...)` / epsilon rules.
The next production-worthy step is a focused `layout-base` clipping/repulsion parity test and then
a narrow `manatee` correction that survives full Architecture verification.

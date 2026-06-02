# HPD-050 - Architecture Junction Final Elements

Date: 2026-06-03

## Context

`stress_architecture_junction_fork_join_026` is still the largest active Architecture
`parity-root` residual. Earlier work already established that the stored CLI upstream fixture is
reproducible and that the manual browser probe is diagnostic-only. This pass rechecked the row with
the enhanced `finalElements` probe plus Rust-side FCoSE input debug before changing any renderer or
solver behavior.

## Evidence

- Current focused compare remains an expected root-only failure:
  - upstream `2808.127x2557.535`
  - local `2822.102x2545.033`
  - width delta `+13.976px`
- Fresh Edge-backed `check-upstream-svgs` reproduced the stored upstream SVG, so the fixture is not
  stale.
- Browser probe effective config remains source-backed for Mermaid 11.15 Architecture:
  `iconSize=80`, `fontSize=16`, `padding=40`, `randomize=false`, `nodeSeparation=75`,
  `idealEdgeLengthMultiplier=1.5`, `edgeElasticity=0.45`, and `numIter=2500`.
- Browser probe constraints match pinned `architectureRenderer.ts`:
  - horizontal alignments: `["ingress","fork","auth"]` and `["api","join","join","db"]`
  - vertical alignments: `["fork","api"]` and `["auth","join","join","cache"]`
  - 9 relative-placement rows, including duplicated `join -> db` and `join -> cache`
- Pinned Mermaid source confirms junction parents are taken directly from `junction.in`; this
  fixture's junctions stay unparented. It also confirms the same-parent/cross-parent FCoSE
  callbacks:
  - same parent ideal length: `idealEdgeLengthMultiplier * iconSize`
  - cross parent ideal length: `0.5 * iconSize`
  - same parent elasticity: configured `edgeElasticity`
  - cross parent elasticity: `0.001`
- Rust debug confirms the same source inputs reach `manatee`:
  - nodes `7`, edges `7`, compounds `2`
  - default edge length `51.428571`
  - compound padding `40`
  - the same duplicate relative-placement constraints
  - one same-parent edge at base ideal length `120` / elasticity `0.45`; the remaining cross-group
    edges at base ideal length `40` / elasticity `0.001`
- The refreshed manual probe still does not reproduce the CLI fixture. It reports final service
  positions closer to local than to stored upstream, while `check-upstream-svgs` reproduces stored
  upstream exactly. Therefore the probe is useful for phase inspection but not authoritative for
  target geometry.

## Outcome

No production change was made. The row is classified as source-input-matched
manatee-vs-Cytoscape FCoSE solution/internal phase residual, with the probe/CLI harness split still
present. Do not change junction parenting, duplicate relative constraints, group rect translation,
edge path emission, root finalization, or one-off text/group constants for this row alone.

The valid future path is a source-backed `manatee` / `cytoscape-fcose` / `cose-base` reference
harness focused on internal FCoSE phases such as intergraph ideal-edge-length adjustment, force
iterations, and compound-bound updates. Do not fit `manatee` to the manual probe when it disagrees
with the CLI fixture.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_final_elements_current.md`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_junction_fork_join_026 > target/compare/arch_junction_fork_join_probe_hpd050_final_elements.json`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3`
- `$env:MERMAN_ARCH_DEBUG_FCOSE_CONSTRAINTS='1'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_constraints_debug.md`
- `$env:MERMAN_ARCH_DEBUG_FCOSE_CONSTRAINTS='1'; $env:MANATEE_FCOSE_DEBUG_EDGE_LENGTHS='1'; cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_edge_lengths_debug.md`

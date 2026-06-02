# HPD-050 Probe Harness Correction

Date: 2026-06-02

## Context

`stress_architecture_junction_fork_join_026` exposed a useful trap: the saved manual browser probe
and the full Mermaid CLI baseline renderer are not the same evidence source. The stored upstream
fixture is reproducible by `check-upstream-svgs`, while the manual probe remains closer to local
Rust output.

## Findings

- `tools/mermaid-cli/node_modules/mermaid/package.json` is `mermaid@11.15.0`.
- The installed `dist/mermaid.js` used by upstream SVG generation does not contain the later
  `withSeededRandom` Architecture seed helper visible in `repo-ref/mermaid` source. A temporary
  `manatee` mulberry32 experiment was therefore rejected and reverted before commit.
- The manual probe was still useful, but its header overstated its authority. It manually
  reconstructs Cytoscape/FCoSE inputs after parsing with Mermaid; it is not equivalent to a full
  Mermaid CLI render.

## Change

- Renamed the probe's page prelude helper to `deterministicPagePreludeScript`.
- Added the same `crypto.getRandomValues` deterministic patch used by xtask's seeded upstream SVG
  wrapper.
- Read shipped Architecture FCoSE config fields from `db.getConfigField(...)` instead of hard-coded
  same-group ideal length, elasticity, randomize, node separation, and iteration values.
- Updated probe comments to say the script is diagnostic-only.

## Evidence

- `cargo fmt --all`
- `cargo test -p manatee xorshift64star_next_f64_unit_matches_seeded_upstream_baseline --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_junction_fork_join_026 > target/compare/arch_junction_fork_join_probe_hpd050_debug_tool_refresh.json`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug_tool_refresh.md`
  expected failure stayed at upstream `2808.127px` vs local `2822.102px`.

## Outcome

No production renderer behavior changed. The manual probe is now better labeled and slightly closer
to the current baseline wrapper, but it still must not be treated as authoritative when it
disagrees with `check-upstream-svgs`.

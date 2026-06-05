# HPD-050 - Architecture Root Tail Edge Attribution

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After the edge curve-style input-model fix, `stress_architecture_nested_groups_002` and
`stress_architecture_batch6_init_fontsize_icon_size_wrap_093` both remained at a `2.5px`
Architecture root-width residual. The remaining question was no longer whether `093` was using the
wrong edge-label anchor; it was which final SVG root edges actually own the remaining width tail.

This pass is diagnostic-only. It changes `debug-architecture-delta` Markdown output and test
coverage, not Architecture layout, rendering, root overrides, baselines, or final SVG output.

## Change

- Added a `Root viewport edge attribution` table to the render-path join in
  `debug-architecture-delta`.
- The table compares actual render-path/local SVG root viewBox edges with the group/service owner
  edge that drives each side of the root bbox.
- Service contributors are expanded from SVG service positions using the local service body
  dimensions, so top-level service-owned root edges are visible beside group-owned edges.
- Updated `architecture_render_path_join_reports_local_deltas` to cover the new table and a
  service-owned root min edge.

## Findings

- `093` is now a final group-edge tail:
  - root left delta is `+2.730461px`, owned by `group-left`;
  - root right delta is `+0.230461px`, owned by `group-right`;
  - the emitted width delta is therefore `0.230461 - 2.730461 = -2.5px`.
- `002` is a mixed top-level service/group root edge tail:
  - root left delta is `+1.250000px`, owned by `service-ingress`;
  - root right delta is `+3.750000px`, owned by `group-platform`;
  - the emitted width delta is therefore `3.75 - 1.25 = +2.5px`.
- Both rows keep stable root padding on the owning edges (`~30px` for `093`, `~40px` for `002`),
  so this evidence does not support a root-padding constant change.

## Evidence

- `target/compare/architecture-delta-root-tail-attribution-002-093-hpd050`
- `target/compare/architecture-report-parity-root-tail-attribution-hpd050`
- `crates/xtask/src/cmd/debug/architecture.rs`

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask -E 'test(architecture_render_path_join_reports_local_deltas) or test(architecture_probe_join_decomposes_group_and_service_bounds)'` - passed, `2` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-root-tail-attribution-hpd050` - passed.
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --probe-dir F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-source-frame-002-093-hpd050 --out target\compare\architecture-delta-root-tail-attribution-002-093-hpd050` - passed.

## Boundary

Do not use this table as a production formula by itself. It identifies which final SVG root edges
own the remaining `2.5px` tails; the next production-capable seam still needs a source-backed
reason for why those owner edges differ.

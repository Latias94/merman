# HPD-050 - Architecture Isolated Service Root Bounds

Date: 2026-06-02
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous disconnected-islands audit rejected a global top-level-service switch from
`svg_root_bounds` to `cytoscape_group_child_bounds`: it made
`stress_architecture_disconnected_islands_046` exact, but expanded full Architecture root
mismatches from `26` to `84`.

That failure is the useful signal. Top-level services do not have one universal root-label phase.
Connected services and simple singleton/iconText rows still need the SVG root `createText(...)`
estimate, while an isolated top-level service inside a diagram that also contains groups behaves as
a separate component for this residual.

## Implementation

- Added `architecture_top_level_service_root_bounds(...)` in
  `crates/merman-render/src/architecture_metrics.rs`.
- Kept the default top-level service contribution as `svg_root_bounds`.
- Used `cytoscape_group_child_bounds` only when both conditions are true:
  - the diagram has one or more groups,
  - the top-level service has no incident edge.
- Left grouped service content bounds on the emitted icon geometry path.

This is intentionally narrower than the rejected experiment. It does not claim browser-exact text
measurement; it preserves the existing headless phase split and makes one source/evidence-backed
root contribution decision explicit.

## Evidence

- Focused `stress_architecture_disconnected_islands_046` root comparison is now exact:
  upstream `823.346x768.460`, local `823.346x768.460`.
- Full Architecture structural `parity` remained green.
- Full Architecture `parity-root` remains an expected failure, but the mismatch count moved from
  `26` to `25`.
- The fixed disconnected-islands row no longer appears in the full root mismatch list.
- Remaining top rows are unchanged residual families, led by:
  - `stress_architecture_junction_fork_join_026` at `+13.976px`,
  - `stress_architecture_batch5_long_titles_and_punct_076` at `+5.000px`,
  - `stress_architecture_html_titles_and_escapes_041` at `+5.000px`.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render architecture_top_level_service_root_bounds_splits_isolated_group_component_phase --lib`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_isolated_root_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_isolated_service_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_isolated_root_bounds.md`
- `Select-String` count on the full root report: `25` dom mismatches.

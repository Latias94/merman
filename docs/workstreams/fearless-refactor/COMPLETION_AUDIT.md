# Fearless Refactor Completion Audit

This audit maps the active objective to concrete evidence so the workstream can track what is
done, what is verified, and what still needs attention.

## Objective

Ship a cleaner, typed-first, parity-safe merman release with fewer duplicated pipelines,
healthier feature gates, modular text/render subsystems, and measurable performance confidence.
Progress is tracked in the fearless-refactor workstream docs.

## Checklist

| Requirement | Current evidence | Status |
| --- | --- | --- |
| Fewer duplicated pipelines | `MILESTONES.md` records the typed render-model migrations for sequence, kanban, gantt, pie, packet, timeline, journey, requirement, sankey, radar, info, zenuml, quadrant chart, gitGraph, treemap, block, er, c4, and xychart. | Met |
| Healthier feature gates | `README.md`, `GATES.md`, and `MILESTONES.md` all document `cargo clippy` and `cargo run -p xtask -- verify --strict` as release-level gates. The latest `cargo run -p xtask -- verify --strict` passed after the flowchart helper rechecks were restored. | Met |
| Modular text subsystem | `MILESTONES.md` records the `text.rs` split into `text/*`, including markdown, measurement, font metrics, and overrides ownership boundaries. | Met |
| Modular renderer subsystems | `MILESTONES.md` records the class, sequence, architecture, and flowchart renderer splits into smaller owner modules. | Met |
| Parity safety | The latest `cargo run -p xtask -- verify --strict` passed after the flowchart basis helper cleanup, and the degenerate-path and cluster-run helpers still guard real mismatches. | Met |
| Measurable performance confidence | `docs/performance/*.md` includes the current baseline, typed-model spotchecks, the mmdr comparison/stage-attribution reports, and the full benchmark gate record. | Met |
| Workstream tracking | `TODO.md`, `MILESTONES.md`, `CHANGELOG.md`, and this audit are kept current. | Met |

## Prompt-to-Artifact Map

| Prompt / requirement | Artifact or command | State |
| --- | --- | --- |
| Typed-first pipeline | `docs/workstreams/fearless-refactor/MILESTONES.md`, `RENDER_MODEL_INVENTORY.md`, `TYPED_RENDERER_GUIDE.md` | Covered |
| Parity-safe release | `cargo run -p xtask -- verify --strict` | Covered |
| Clippy in success criteria | `GATES.md`, `README.md`, `MILESTONES.md` | Covered |
| Performance evidence | `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md`, `docs/performance/spotcheck_2026-05-10_full_bench_gate.md`, `docs/performance/COMPARISON.md` | Covered |
| Override debt governance | `OVERRIDE_FOOTPRINT.md`, `OVERRIDE_POLICY.md`, `cargo run -p xtask -- report-overrides --check-no-growth` | Covered |
| Delete obsolete code | flowchart helper rechecks in `TODO.md` and `CHANGELOG.md`, plus the basis helper cleanup in `crates/merman-render/src/svg/parity/flowchart/edge_geom/basis.rs` | Covered for the recheck decision; obsolete helpers were removed after strict-gate parity stayed green, while the degenerate-path and cluster-run helpers remain in place where parity still fails |
| Keep docs current | `TODO.md`, `MILESTONES.md`, `CHANGELOG.md` | Covered |

## What Was Verified Recently

- `cargo run -p xtask -- verify --strict` passed after the flowchart helper restorations, the cyclic-special basis helper removal, and the single-use midpoint helper inline.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passed during the helper rechecks.
- Flowchart DOM spotchecks for `edges_to_from_subgraphs`, `subgraph_spec`, and `cluster` were green while the helpers were temporarily disabled, and the later strict-gate run showed that the cyclic-special basis helper could stay deleted while the degenerate-path and cluster-run helpers remain required for full parity.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3` and `cargo run -p xtask -- verify --strict` stayed green after inlining the State viewport mode helper.
- State raw/non-raw context resolution cleanup kept `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`, `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, and `cargo run -p xtask -- verify --strict` green.
- State label HTML helper cleanup kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`, and `cargo run -p xtask -- verify --strict` green.
- State link sanitizer visibility cleanup kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`, and `cargo run -p xtask -- verify --strict` green.
- Shared RoughJS parity helper extraction kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`, `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter flowchart`, and `cargo run -p xtask -- verify --strict` green.
- Shared RoughJS rectangle and circle generation extraction kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`, `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter flowchart`, and `cargo run -p xtask -- verify --strict` green.
- Flowchart RoughJS op-set serializer cleanup kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter flowchart`, and `cargo run -p xtask -- verify --strict` green.
- Flowchart RoughJS dash parsing and node helper visibility cleanup kept `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter flowchart`, and `cargo run -p xtask -- verify --strict` green.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3` failed when the C4 root lookup was bypassed, and `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3` failed when the Timeline root lookup was bypassed, so both tables remain required.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3` failed when the State root lookup was bypassed, so that 54-entry table also remains required.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3` failed when the C4 root viewport lookup was bypassed, so the 35-entry C4 root table remains required.

## Remaining Gaps

- `TODO.md` still keeps `Add parse/render timing samples before and after each typed migration` open.
- `TODO.md` still keeps `Delete overrides made obsolete by typed model or measurement fixes` open.

## Conclusion

The workstream is structurally in good shape, but the release objective is not complete yet.
The remaining work is about filling the benchmark gap and continuing the M5 override reduction
pass, not about reopening the already-passed strict parity gate.

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
| Fewer duplicated pipelines | `MILESTONES.md` and `RENDER_MODEL_INVENTORY.md` record all non-error in-tree Mermaid diagrams as typed-first on the render path; the JSON render fallback is now limited to `error` and custom registry parsers. | Met |
| Healthier feature gates | `GATES.md` and `MILESTONES.md` now document `cargo run -p xtask -- verify --feature-matrix`; `--strict` includes that matrix for `merman` no-default/render/raster and `merman-core` no-default, alongside all-features check and clippy. | Met |
| Modular text subsystem | `MILESTONES.md` records the `text.rs` split into `text/*`, including markdown, measurement, font metrics, and overrides ownership boundaries. | Met |
| Modular renderer subsystems | `MILESTONES.md` records the class, sequence, architecture, and flowchart renderer splits into smaller owner modules. | Met |
| Parity safety | The latest `cargo run -p xtask -- verify --strict` passed on 2026-05-11 after the ER text override generator cleanup; degenerate-path and cluster-run helpers still guard real mismatches. | Met |
| Measurable performance confidence | `docs/performance/*.md` includes the current baseline, typed-model spotchecks, the mmdr comparison/stage-attribution reports, the typed migration timing index, and the full benchmark gate record. | Met |
| Workstream tracking | `TODO.md`, `MILESTONES.md`, `CHANGELOG.md`, and this audit are kept current. | Met |

## Prompt-to-Artifact Map

| Prompt / requirement | Artifact or command | State |
| --- | --- | --- |
| Typed-first pipeline | `docs/workstreams/fearless-refactor/MILESTONES.md`, `RENDER_MODEL_INVENTORY.md`, `TYPED_RENDERER_GUIDE.md` | Covered |
| Parity-safe release | `cargo run -p xtask -- verify --strict` | Covered |
| Public feature gates | `cargo run -p xtask -- verify --feature-matrix` and `cargo run -p xtask -- verify --strict` | Covered |
| Clippy in success criteria | `GATES.md`, `README.md`, `MILESTONES.md` | Covered |
| Performance evidence | `docs/workstreams/fearless-refactor/TYPED_MIGRATION_TIMING.md`, `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md`, `docs/performance/spotcheck_2026-05-10_full_bench_gate.md`, `docs/performance/COMPARISON.md` | Covered |
| Override debt governance | `OVERRIDE_FOOTPRINT.md`, `OVERRIDE_POLICY.md`, `cargo run -p xtask -- report-overrides --check-no-growth` | Covered |
| Delete obsolete code | flowchart helper rechecks in `TODO.md` and `CHANGELOG.md`, the basis helper cleanup in `crates/merman-render/src/svg/parity/flowchart/edge_geom/basis.rs`, and deletion of the stale ER text override generator | Covered for the recheck decision; obsolete helpers/generators were removed after strict-gate parity stayed green, while the degenerate-path and cluster-run helpers remain in place where parity still fails |
| Keep docs current | `TODO.md`, `MILESTONES.md`, `CHANGELOG.md`, `GATES.md`, and `OVERRIDE_POLICY.md` | Covered |

## What Was Verified Recently

- `cargo run -p xtask -- verify --strict` passed on 2026-05-11 after removing the stale
  `xtask gen-er-text-overrides` command/generator and the empty ER `calcTextWidth` lookup path.
  A later empty-diagram root viewport cleanup lowered the root budget to `750`, and the later
  Class cleanup lowered the text lookup budget to `517`.
- A Block text audit tightened `OVERRIDE_POLICY.md` and `GATES.md`: layout-affecting text lookup
  deletion now requires layout snapshot evidence because the default deterministic layout measurer
  can still differ when the vendored SVG/HTML measurer matches the stored override.
- Follow-up Block and State text audits found zero exact `DeterministicTextMeasurer` width matches
  in the remaining audited lookup buckets, so those pruning tracks are paused until shared
  deterministic measurement improves.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo clippy -p xtask --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render`, and
  `cargo run -p xtask -- report-overrides --check-no-growth` passed after removing the generated
  module's `clippy::all` umbrella allowance and synchronizing the font-metrics generator template.
- `cargo run -p xtask -- verify --strict` also passed on 2026-05-11 after the Class `OK`,
  `ApiClient`, and `ERROR` cleanup passes, the dense layout golden refresh, and tightening the
  text lookup no-growth budget to `519`.
- The follow-up Class `Payment` cleanup removed two more width overrides, refreshed the affected
  layout golden, and tightened the text lookup no-growth budget again to `517`.
- The follow-up Class `Cart` cleanup removed one more `calcTextWidth` override without golden
  drift and tightened the text lookup no-growth budget again to `516`.
- The follow-up Class `Server` cleanup removed one rendered width override, refreshed the affected
  style layout golden, and tightened the text lookup no-growth budget again to `515`; its
  `calcTextWidth` cap remains because a focused SVG test still asserts Mermaid's `max-width: 92px`.
- A focused cap recheck rejected the `DB` `calcTextWidth` deletion because Mermaid's
  `max-width: 72px` is still asserted by the same Class SVG test.
- The follow-up Class `Dog` and `Mineral` cleanups removed two more `calcTextWidth` overrides
  without layout drift and tightened the text lookup no-growth budget again to `513`; the `Mineral`
  rendered width override remains because deleting it shifts the upstream root `max-width`.
- The follow-up Class `Duck` cleanup removed two width overrides, refreshed the affected Duck
  layout goldens, and tightened the text lookup no-growth budget again to `511`.
- The follow-up Class `Item` and `Order` cleanup removed two more width overrides, refreshed the
  affected parallel-edges layout golden, and tightened the text lookup no-growth budget again to
  `509`.
- The follow-up Class `Wheel` cleanup removed one more rendered width override, refreshed the
  affected relation-types layout golden, and tightened the text lookup no-growth budget again to
  `508`; `Fish` was retained because it still guards docs class root `max-width` parity.
- The follow-up Class `connects` cleanup removed one relation-label rendered width override,
  refreshed the affected style layout golden, and tightened the text lookup no-growth budget again
  to `507`.
- The follow-up Class `builds` cleanup removed one relation-label rendered width override,
  refreshed the affected dense-namespaces and notes-wrap layout goldens, and tightened the text
  lookup no-growth budget again to `506`.
- The follow-up Class `parses` cleanup removed one relation-label rendered width override,
  refreshed the affected dense-namespaces layout golden, and tightened the text lookup no-growth
  budget again to `505`.
- The follow-up Class `emits` cleanup removed one relation-label rendered width override,
  refreshed the affected many-relations layout golden, and tightened the text lookup no-growth
  budget again to `504`.
- The follow-up Class `feedback` cleanup removed one relation-label rendered width override,
  refreshed the affected many-relations layout golden, and tightened the text lookup no-growth
  budget again to `503`.
- The follow-up Class `returns` cleanup removed one relation-label rendered width override,
  refreshed the affected dense-namespaces, enums-and-interfaces, and nested-generics layout
  goldens, and tightened the text lookup no-growth budget again to `502`.
- The follow-up Class `wraps` cleanup removed one relation-label rendered width override,
  refreshed the affected dense-namespaces layout golden, and tightened the text lookup no-growth
  budget again to `501`.
- The follow-up Class `reads` cleanup removed one relation-label rendered width override,
  refreshed the affected many-relations and styles layout goldens, and tightened the text lookup
  no-growth budget again to `500`.
- The M2 typed-model milestone was reconciled with `RENDER_MODEL_INVENTORY.md`: all non-error
  in-tree diagrams are typed-first, and remaining work is M5 override reduction rather than
  another JSON-to-typed migration.
- `cargo nextest run -p merman-render --test class_svg_test`,
  `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`,
  `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `cargo nextest run -p merman-render --test layout_snapshots_test
  fixtures_match_layout_golden_snapshots_when_present`, and
  `cargo run -p xtask -- report-overrides --check-no-growth` passed for the Class pruning pass.
- The remaining exact Class `calcTextWidth` matches `bar()`, `E`, `IService`,
  `+run() : Status`, `Client`, `+start()`, and `API` were kept because focused SVG tests or
  layout snapshot evidence still assert those Mermaid HTML `max-width` caps explicitly.
- `cargo run -p xtask -- verify --strict` passed after Flowchart text override pruning and the State Dagre input builder cleanup.
- `cargo run -p xtask -- verify --feature-matrix` passed, covering `merman` no-default/render/raster and `merman-core` no-default feature checks.
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

- `TODO.md` still keeps `Delete overrides made obsolete by typed model or measurement fixes` open.

## Conclusion

The workstream is structurally in good shape, but the release objective is not complete yet.
The remaining work is about continuing the M5 override reduction pass, not about reopening the
already-passed strict parity gate.

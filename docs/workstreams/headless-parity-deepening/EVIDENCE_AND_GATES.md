# Headless Parity Deepening - Evidence And Gates

Status: Active
Last updated: 2026-06-02

## HPD-020 - Baseline Registry

Outcome:

- Added [crates/merman-core/src/baseline.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/baseline.rs)
  as the explicit baseline truth seam for:
  - pinned Mermaid tag
  - pinned Mermaid version
  - pinned Mermaid version suffix
  - explicit legacy generated suffix
- `Engine::default()` now uses pinned-baseline registry constructors rather than
  `default_mermaid_11_12_2*`.
- `DetectorRegistry`, `DiagramRegistry`, and `RenderDiagramRegistry` now expose pinned-baseline
  constructors as the live API, with `default_mermaid_11_12_2*` retained only as deprecated
  compatibility aliases.
- xtask importers, bench entrypoints, and baseline report labeling now route through the pinned
  baseline path instead of presenting `11.12.x` names as current truth.
- Historical generated filenames and lookup modules are still legacy-suffixed; that is now explicit
  provenance, not the live baseline identity.

Touched production surfaces:

- [crates/merman-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/lib.rs)
- [crates/merman-core/src/detect/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/detect/mod.rs)
- [crates/merman-core/src/diagram/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagram/mod.rs)
- [crates/xtask/src/cmd/overrides/report.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/overrides/report.rs)
- [crates/xtask/src/cmd/root_override_audit.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/root_override_audit.rs)
- [crates/xtask/src/cmd/import/docs.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/docs.rs)
- [crates/xtask/src/cmd/import/examples.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/examples.rs)
- [crates/xtask/src/cmd/import/html.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/html.rs)
- [crates/xtask/src/cmd/import/pkg_tests.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/pkg_tests.rs)
- [crates/xtask/src/cmd/import/cypress.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/cypress.rs)
- [crates/merman/benches/pipeline.rs](/F:/SourceCodes/Rust/merman/crates/merman/benches/pipeline.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-core baseline --lib`
- `rg -n "default_mermaid_11_12_2|default_mermaid_11_12_2_full|default_mermaid_11_12_2_tiny" crates -g '!target/**'`

Verification notes:

- The remaining `default_mermaid_11_12_2*` hits are compatibility alias definitions only; live
  call sites have been moved to the pinned-baseline constructors.
- An attempted `cargo test -p xtask ... --lib` check was invalid because `xtask` has no library
  target. The baseline-report path was instead verified by code inspection plus the existing
  `pinned_mermaid_baseline_label_reads_lockfile_ref` unit test in
  `crates/xtask/src/cmd/overrides/report.rs`.

## HPD-030 - Residual Taxonomy

Outcome:

- Added an explicit six-class residual taxonomy to
  [docs/workstreams/headless-parity-deepening/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/DESIGN.md):
  - source-backed behavior gap
  - generated measurement gap
  - browser lattice tail
  - stale baseline or stale override
  - solver / phase residual
  - scope boundary
- Applied that taxonomy to the active root residual lane in
  [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md).
- Froze the intended interpretation of the current active buckets:
  - Flowchart: mostly browser lattice tails
  - Architecture: main solver/phase residual front, plus some measurement tails
  - Sequence: mixed generated-measurement gaps and browser tails
  - Class: generated-measurement gap plus stale-table audit front
  - Timeline/Journey: browser tails unless stronger evidence emerges

Validation basis:

- Current active counts and classifications were derived from:
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md)
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md)
  - [docs/alignment/STATUS.md](/F:/SourceCodes/Rust/merman/docs/alignment/STATUS.md)
  - [docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md](/F:/SourceCodes/Rust/merman/docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md)

Verification notes:

- This task intentionally does not claim new residual counts beyond the current authoritative
  workstream state.
- The deliverable is a durable classification system and queue-shaping mapping, not a fresh one-off
  report or a pseudo-precise completion metric.

## HPD-040 - Measurement / Root Bounds Platform

Outcome:

- Moved the SVG emitted-bounds scanner from the State renderer submodule into
  [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs).
  This matches its real ownership: it is shared root-bounds infrastructure for State,
  Architecture, and GitGraph, not State-specific rendering logic.
- Extracted Sequence note final wrapping into `sequence_note_final_wrapped_lines(...)` and final
  wrapped-text measurement into `measure_sequence_note_final_text(...)`.
- Reused the Sequence note final wrap path from layout, root-bounds, and SVG rendering so Mermaid
  source-backed note wrapping is not re-derived in three places.
- Removed the now-unneeded crate-level re-export of Sequence note slack constants. The constants
  remain internal to the Sequence seam and its tests.
- Added no new fixture-keyed text tables, root overrides, or ad hoc parity constants.

Touched production surfaces:

- [crates/merman-render/src/svg/parity.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity.rs)
- [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs)
- [crates/merman-render/src/svg/parity/state/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/state/mod.rs)
- [crates/merman-render/src/sequence.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence.rs)
- [crates/merman-render/src/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/notes.rs)
- [crates/merman-render/src/sequence/root_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/root_bounds.rs)
- [crates/merman-render/src/svg/parity/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/notes.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render svg_emitted_bounds --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo run -p xtask -- report-overrides --check-no-growth`

Negative / residual evidence:

- `cargo nextest run -p merman-render svg_emitted_bounds` was attempted first but the local toolchain
  does not have `cargo-nextest` installed, so verification used `cargo test`.
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_root_width --test sequence_svg_test`
  still fails. This is intentional evidence that HPD-040 did not claim forced root parity closure.
- Deterministic CLI render for
  `upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026`
  produced `max-width: 570px` / `viewBox="-170 -10 570 412"` while the upstream target remains
  `566px`.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026 --report-root --report-root-all`
  reported headless vendored local `585.000px` vs upstream `566.000px` (`+19.000px`). Treat this as
  a Sequence measurement/root residual for later classification, not as a reason to add a local
  width override.

## HPD-050 - Architecture Layout Engine Audit

First slice outcome:

- Extracted `architecture_fcose_prelayout_bounds(...)` from the main Architecture layout function.
  The helper owns the Mermaid/Cytoscape pre-layout `eles.boundingBox()` approximation that produces:
  - the initial FCoSE coordinate-frame center,
  - per-node `BoundsExtras` fed into `manatee`,
  - top-level compound bbox selection with configured Architecture padding.
- Removed the layout-view group-title field. Current source/evidence says group titles are rendered
  inside compound bounds and do not participate in the pre-layout center used for FCoSE relocation;
  final SVG rendering still reads titles from semantic model data.
- Added direct unit coverage for the pre-layout helper so the service label bottom extra and
  coordinate-frame center stay explicit.
- This is an adapter-boundary refactor, not a residual-count claim. It keeps Mermaid/Cytoscape
  approximation policy in `merman-render` instead of leaking another diagram-specific rule into
  `manatee`.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_prelayout_bounds_feed_label_extras_without_group_title_state --lib`
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops --lib`
- `cargo test -p merman-render --test architecture_layout_test`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual evidence:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_after_prelayout_adapter.md`
  still fails with the known root-only tail: upstream `542.926px`, local `547.926px`, delta
  `+5.000px`.
- The unchanged focused tail is intentional evidence that this pass moved ownership boundaries
  without silently tuning Architecture root widths.

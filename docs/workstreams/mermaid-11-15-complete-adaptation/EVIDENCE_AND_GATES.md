# Mermaid 11.15 Complete Adaptation - Evidence And Gates

Status: Active
Last updated: 2026-05-31

## Smallest Current Repro

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

On 2026-05-31 this failed, proving the implemented matrix is not yet verifiably green against the
current stored upstream SVG baselines.

## Gate Set

### Baseline And Artifact Gates

```bash
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify-generated
```

### Full SVG Parity Gates

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

`parity` is the first closeout gate. `parity-root` should be run after structural parity is green or
after a scoped root-viewport task needs evidence.

### Package And Workspace Gates

Use targeted tests for touched packages, then the workspace gate near closeout:

```bash
cargo nextest run -p xtask
cargo nextest run -p merman-core
cargo nextest run -p merman-render
$env:CARGO_PROFILE_TEST_DEBUG='0'; $env:CARGO_BUILD_JOBS='2'; cargo nextest run --workspace
```

### Formatting And Diff Gates

```bash
cargo fmt --check
git diff --check
```

## Evidence Log

- 2026-05-31 lane opening:
  - Goal: close Mermaid 11.15 complete-adaptation campaign by making the implemented diagram matrix
    green against Mermaid 11.15 SVG baselines and recording all remaining family decisions.
  - `git status --short`: clean before opening docs.
  - `cargo run -p xtask -- check-alignment`: passed.
  - `cargo run -p xtask -- verify-generated`: passed.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    failed.
  - Failure count from generated parity reports: 525 DOM mismatches across 8 diagram groups:
    sequence=322, timeline=91, c4=51, journey=26, sankey=24, class=9, flowchart=1, xychart=1.
  - Dominant failure family: marker ID prefix drift where stored upstream SVG baselines use bare
    IDs such as `arrowhead`, while local 11.15-oriented output uses `<svg-id>-arrowhead`.
  - Additional likely residuals: Sankey stroke-width/layout deltas, Class hierarchical namespace
    DOM deltas, Flowchart MathML `columnalign`, and XYChart data-label color.
  - Active compare reports still label upstream SVG baselines as Mermaid 11.12.3, so baseline
    metadata must be fixed before treating all mismatches as renderer defects.
- 2026-05-31 M15C-020 inventory:
  - `docs/workstreams/mermaid-11-15-complete-adaptation/PARITY_FAILURE_INVENTORY.md` records the
    current 525-mismatch split.
  - Stale-baseline dominated batch: sequence, timeline, c4, journey (490 mismatches).
  - Fresh-baseline-before-code batch: sankey, class, xychart.
  - Targeted residual candidate: flowchart MathML `columnalign`.
- 2026-05-31 M15C-030 active metadata cleanup:
  - Compare report headers in `crates/xtask/src/cmd/compare/diagrams/*.rs` now refer to the
    `pinned Mermaid baseline` instead of hard-coded Mermaid 11.12.3.
  - `docs/rendering/SVG_CANONICAL_XML.md` now points to the baseline pinned in
    `tools/upstreams/REPOS.lock.json`.
  - `docs/alignment/PARITY_HARDENING_PLAN.md` now names Mermaid `@11.15.0` as the current baseline
    and keeps 11.12.3 labels as historical snapshots.
  - `rg "Mermaid 11\\.12\\.3|Mermaid CLI pinned to Mermaid 11\\.12\\.3|\\(Mermaid 11\\.12\\.3\\)" crates/xtask/src/cmd/compare/diagrams docs/rendering/SVG_CANONICAL_XML.md docs/alignment/PARITY_HARDENING_PLAN.md -n`:
    no matches.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - `cargo nextest run -p xtask`: passed, 67 tests.
  - `cargo run -p xtask -- check-alignment`: passed.
- 2026-05-31 M15C-040 sequence 11.15 probe and central-connection fix:
  - `node -e "console.log(require('./tools/mermaid-cli/node_modules/mermaid/package.json').version)"`:
    printed `11.15.0`.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter basic --out target/upstream-svgs-11-15-probe`:
    generated fresh 11.15 sequence `basic` probe SVGs.
  - Initial fresh `basic` compare failed only on
    `sequence/upstream_docs_sequencediagram_basic_syntax_035`, proving the stored sequence
    baselines were stale but also exposing a real central-connection parser/model gap.
  - Implemented sequence central connections as upstream does: normalized actor ids, numeric
    `centralConnection` on visible messages, and internal type 59/60 control messages. This also
    fixed resulting SVG message `data-id` values (`i0`, `i2`, `i4`) and central circle DOM.
  - Added 11.15 sequence marker defs (`solidTopArrowHead`, `solidBottomArrowHead`,
    `stickTopArrowHead`, `stickBottomArrowHead`), scoped sequence symbol ids, and 11.15 sequence
    data attributes for participants, lifelines, messages, and notes.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --filter basic --upstream-root target/upstream-svgs-11-15-probe --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter central --out target/upstream-svgs-11-15-central`:
    generated fresh 11.15 central-connection SVGs.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram sequence --filter central --upstream-root target/upstream-svgs-11-15-central --dom-mode parity --dom-decimals 3`:
    passed.
  - `cargo nextest run -p merman-core sequence`: passed, 32 tests.
  - `cargo nextest run -p merman-render sequence`: passed, 16 tests.
  - `cargo nextest run -p merman-core fixtures_match_golden_snapshots`: passed after refreshing only
    central-connection semantic goldens.
  - `cargo nextest run -p merman-render fixtures_match_layout_golden_snapshots_when_present`:
    passed after refreshing only central-connection layout goldens.
  - `cargo fmt --check`: passed.
  - Non-sequence marker-ID baseline refresh remains open for C4, Journey, and Timeline; stored
    `fixtures/upstream-svgs/sequence` was not bulk-refreshed in this slice.

## Evidence Anchors

- `docs/workstreams/mermaid-11-15-complete-adaptation/DESIGN.md`
- `docs/workstreams/mermaid-11-15-complete-adaptation/TODO.md`
- `docs/workstreams/mermaid-11-15-complete-adaptation/PARITY_FAILURE_INVENTORY.md`
- `docs/alignment/STATUS.md`
- `docs/rendering/UPSTREAM_SVG_BASELINES.md`
- `tools/upstreams/REPOS.lock.json`
- `target/compare/*_report_parity.md`

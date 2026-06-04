# Theme Capability Deepening - Evidence And Gates

Status: Closed
Last updated: 2026-06-04

## Smallest Current Repro

```bash
cargo nextest run -p merman-render chart_palette
cargo nextest run -p merman-render xychart
cargo nextest run -p merman-render quadrantchart
```

## Gate Set

### TCD-020 Targeted Iteration Gate

```bash
cargo fmt --check
cargo nextest run -p merman-render flowchart_svg
cargo nextest run -p merman-render class_svg
cargo nextest run -p merman-render state_svg
cargo nextest run -p merman-render sequence_svg
cargo nextest run -p merman-render block_svg
cargo nextest run -p merman-render presentation_theme
```

What this proves:

- migrated CSS providers still emit expected Mermaid-owned theme surfaces;
- shared fallback logic did not break the first high-duplication families;
- the seam remains renderer-internal.

### TCD-030 Targeted Iteration Gate

```bash
cargo fmt --check
cargo nextest run -p merman-render chart_palette
cargo nextest run -p merman-render xychart
cargo nextest run -p merman-render quadrantchart
```

What this proves:

- chart-style palette derivation remains stable;
- explicit chart/theme overrides still beat derived roles.
- XyChart's renderer-owned helper is covered directly, and related SVG integration tests still
  pass. The literal `quadrantchart_svg` nextest filter currently matches no test names; use
  `quadrantchart` for the focused integration gate.

### Public Coverage Gate

```bash
cargo fmt --check
cargo test -p merman --features render --test theme_renderability_smoke
```

What this proves:

- the new seam still carries visible theme signals through the high-level render API.
- the command runs the actual `theme_renderability_smoke` integration test binary. The earlier
  filter form `cargo test -p merman theme_renderability_smoke --features render` runs zero tests in
  this repo and should not be used as evidence.

### Review Gate

Run `review-workstream` before accepting lane completion. Record any remaining raw-theme escape
hatches and intentional non-migrations here.

## Evidence Anchors

- `docs/workstreams/theme-capability-deepening/DESIGN.md`
- `docs/workstreams/theme-capability-deepening/TODO.md`
- `docs/workstreams/theme-capability-deepening/TASKS.jsonl`
- `docs/workstreams/theme-capability-deepening/CAMPAIGNS.jsonl`
- `docs/workstreams/theme-capability-deepening/MILESTONES.md`
- `docs/adr/0068-render-side-presentation-theme-view.md`
- touched code/test paths under `crates/merman-render/src/svg/parity` and `crates/merman-render/tests`

## Current Evidence Log

- 2026-06-03: Lane opened as the explicit follow-on to `theme-parity` split work and anchored to
  `ARCH-013` plus ADR-0068.
- 2026-06-03: TCD-020 completed. Verified with `cargo fmt --check --all`,
  `cargo nextest run -p merman-render flowchart_svg`, `cargo nextest run -p merman-render class_svg`,
  `cargo nextest run -p merman-render sequence_svg`, `cargo nextest run -p merman-render state_svg`,
  `cargo nextest run -p merman-render block_svg`, `cargo nextest run -p merman-render presentation_theme`,
  and `git diff --check`.
- 2026-06-04: TCD-030 completed. Verified with `cargo fmt --check --all`,
  `cargo nextest run -p merman-render chart_palette`, `cargo nextest run -p merman-render xychart`,
  `cargo nextest run -p merman-render quadrantchart`, and `git diff --check`.
- 2026-06-04: TCD-040 completed by reusing the existing HPD-080 public renderability smoke instead
  of adding redundant fixtures. Verified with
  `cargo fmt --check --all`, `cargo test -p merman --features render --test theme_renderability_smoke`,
  and `git diff --check` after confirming the original filter-form command ran zero tests.
- 2026-06-04: TCD-050 closeout completed. Review found no blocking workstream-compliance or
  code-quality issues. Fresh closeout gate passed with `cargo fmt --check --all`, all focused
  TCD-020/TCD-030 renderer gates, `cargo test -p merman --features render --test theme_renderability_smoke`,
  and `git diff --check`.

## Verification Details

### 2026-06-04 - TCD-030 Fresh Verification

Claim: XyChart plot palette resolution now has one renderer-owned helper, explicit resolved
Mermaid `xyChart.plotColorPalette` still wins, and related SVG render paths remain stable.

Scope:

- `crates/merman-render/src/chart_palette.rs`
- `crates/merman-render/src/xychart.rs`
- related XyChart and QuadrantChart SVG integration tests

Commands and results:

- `cargo fmt --check --all`: PASS
- `cargo nextest run -p merman-render chart_palette`: PASS, 3 tests
- `cargo nextest run -p merman-render xychart`: PASS, 3 tests
- `cargo nextest run -p merman-render quadrantchart`: PASS, 3 tests
- `git diff --check`: PASS

Broader gates skipped:

- `cargo nextest run --workspace`: skipped because TCD-030 only touches internal
  `merman-render` palette resolution and the targeted render-family gates passed.
- `cargo test -p merman theme_renderability_smoke --features render`: deferred to TCD-040, whose
  scope is public renderability/theme coverage.

### 2026-06-04 - TCD-040 Public Renderability Verification

Claim: the new render-side presentation theme and chart palette seams still carry visible theme
signals through the public `HeadlessRenderer` API, and no additional public fixtures are needed for
this slice.

Scope:

- `crates/merman/tests/theme_renderability_smoke.rs`
- `docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md`

Commands and results:

- `cargo test -p merman theme_renderability_smoke --features render`: PASS but ran 0 tests; not
  used as evidence.
- `cargo fmt --check --all`: PASS
- `cargo test -p merman --features render --test theme_renderability_smoke`: PASS, 12 tests.
- `git diff --check`: PASS

Behavior proven:

- Flowchart, Class, State, Sequence, and Block still expose visible theme signals through
  `HeadlessRenderer`, covering the TCD-020 public path.
- XyChart still exposes explicit `xyChart.plotColorPalette` colors through public rendering,
  covering the TCD-030 visible path.
- Existing HPD-080 coverage already proves the relevant public behavior; adding new snapshots would
  not strengthen this task.

### 2026-06-04 - TCD-050 Closeout Verification

Review result:

- Workstream compliance: no blocking findings.
- Code quality: no blocking findings.
- Missing gates: none after correcting stale no-test filter commands.
- Residual risk: this lane does not claim full theme-system completion. Remaining raw-theme access
  in other diagram families is a follow-on boundary.

Fresh closeout commands and results:

- `cargo fmt --check --all`: PASS
- `cargo nextest run -p merman-render flowchart_svg`: PASS, 14 tests
- `cargo nextest run -p merman-render class_svg`: PASS, 19 tests
- `cargo nextest run -p merman-render state_svg`: PASS, 3 tests
- `cargo nextest run -p merman-render sequence_svg`: PASS, 4 tests
- `cargo nextest run -p merman-render block_svg`: PASS, 4 tests
- `cargo nextest run -p merman-render presentation_theme`: PASS, 2 tests
- `cargo nextest run -p merman-render chart_palette`: PASS, 3 tests
- `cargo nextest run -p merman-render xychart`: PASS, 3 tests
- `cargo nextest run -p merman-render quadrantchart`: PASS, 3 tests
- `cargo test -p merman --features render --test theme_renderability_smoke`: PASS, 12 tests
- `git diff --check`: PASS

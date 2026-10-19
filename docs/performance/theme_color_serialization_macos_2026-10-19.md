# Theme color serialization (macOS, 2026-10-19)

Color serialization now writes numeric components directly into the final color String. It removes
temporary channel Strings while preserving the existing Khroma parser, color state and JavaScript
number spelling. This checkpoint admits an allocation/work reduction, not a calibrated latency
improvement or a recovered v0.9.0 regression.

## Cause and boundary

The Kanban preparation profile contains repeated `lighten` calls beneath `KanbanCssBinding::resolve`.
Each changed HSL color previously allocated three numeric Strings before formatting the final
color. RGB, alpha and hex serialization had the same ownership problem. A private Display value
now writes through the existing `ryu_js` formatter; hex writes rounded/clamped bytes directly.
No global cache, new retained state, skipped validation or changed stage boundary is introduced.

The original rounding locations remain: RGB alpha is rounded, HSLA alpha is not. Negative zero,
NaN, infinities, raw unchanged-color preservation, keyword canonicalization and lowercase hex
retain their previous behavior. Existing tests now explicitly distinguish RGB rounding from
the full precision of HSLA alpha.

This shared serializer was identical in v0.9.0. Its cost is therefore not evidence of a theme
refactor regression. Kanban's CSS work also moved from emission to preparation across releases;
the private layout-stage difference cannot be attributed entirely to new work.

## Evidence

The preregistered ledger and raw observations are under
`target/bench/experiments/theme-color-serialization-20261019/`. The base product revision is
`f67a10453`, immediately after the [same-SVG release refresh](theme_v090_same_svg_refresh_macos_2026-10-19.md).
Both sides use release all-feature libraries, identical inputs and five allocation observations
per operation. Compilation, input loading and result serialization are outside the memory window.

- All 17,091 direct color-operation cases preserve exact output and error results. All observations
  are stable; 10,307 cases reduce allocation work, and no case increases any of the four metrics.
  The corpus covers hex, RGB, HSL, keywords, malformed values, finite/nonfinite adjustments, alpha,
  inversion and mixed-space errors.
- All 178 complete public-render controls preserve exact SVG/report bytes and ordered custom
  measurement traces. Of 172 jointly stable rows, 42 reduce allocation count and bytes; six variable
  Flowchart rows remain memory-inconclusive. No stable row increases peak or retained growth.
- Default Kanban decreases from 656 to 620 allocations and 115,916 to 115,694 allocated bytes per
  complete render. Peak growth remains 45,838 bytes and retained growth remains 30,319 bytes.
- All 418 catalog cells across 38 scenes, 35 logical families and 10 presets plus no-theme are
  stable and preserve SVG, reports, outcomes, configuration and catalog metadata. Of these, 67
  reduce allocations across eight logical families; the remaining cells are unchanged. Summing
  each cell's per-operation savings gives 796 allocations and 6,907 bytes, not a single-render
  saving. Peak and retained growth are unchanged throughout.

No calibrated timing run was performed for this candidate. The preceding release diagnostic
measures the earlier revision and must not be presented as a latency result for this change.

## Verification

- `cargo nextest run --locked -p merman-core -p merman-render -p merman --all-features`:
  7,624 passed, seven skipped.
- Production release Clippy for those three crates, all features and `-D warnings`: passed.
- Private theme acceptance: 34 passed. WASM host tests: 47 passed.
- Optimized full WASM build and 37-diagram smoke: passed.
- All-family DOM comparison in structure, parity and parity-root modes: passed.
- Workspace formatting and diff checks: passed.

Full strict verification was not rerun. The preceding family-preparation checkpoint completed
661 feature-matrix builds and 36 artifact checks, then stopped at the missing local Mermaid CLI
`esbuild` dependency. No full-strict certification is claimed here.

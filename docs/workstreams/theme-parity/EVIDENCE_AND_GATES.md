# Theme Parity Refactor - Evidence And Gates

Status: Complete
Last updated: 2026-06-01

## Required Gates

Core theme work:

```sh
cargo fmt
cargo nextest run -p merman-core theme
```

Renderer theme/CSS work:

```sh
cargo fmt
cargo nextest run -p merman-render block_svg class_svg flowchart_svg
```

WASM and frontend theme surface:

```sh
cargo check -p merman-wasm --target wasm32-unknown-unknown
npm run build --prefix platforms/web
npm run build --prefix playground
```

Broad parity gate before closeout:

```sh
cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3
```

This broad fixture gate was split to follow-up work for this lane. The closeout gate for this
theme refactor is the narrower set of targeted Rust, WASM, and frontend commands recorded below.

Strict closeout gate, if cost is acceptable:

```sh
cargo run -p xtask -- verify --strict
```

## Evidence Log

- 2026-06-01: Workstream opened after comparing Merman theme support against Mermaid theme source.
  Current finding: Merman lacks default-theme expansion, has partial preset coverage, and maintains
  separate frontend theme lists.
- 2026-06-01: TPR-020 implemented default theme expansion in `merman-core`.
  Evidence:
  - `cargo fmt` passed.
  - `cargo nextest run -p merman-core theme` passed: 7 tests run, 7 passed.
  - `cargo nextest run -p merman-render block_svg class_svg flowchart_svg` passed: 29 tests run,
    29 passed.
- 2026-06-01: TPR-030 refactored shared theme helpers in `merman-core`.
  Evidence:
  - `cargo fmt` passed.
  - `cargo nextest run -p merman-core theme` passed: 7 tests run, 7 passed.
- 2026-06-01: TPR-040 added `SvgTheme` and migrated Class, Block, and Flowchart CSS callers.
  Evidence:
  - `cargo fmt` passed.
  - `cargo nextest run -p merman-render block_svg class_svg flowchart_svg` passed: 29 tests run,
    29 passed.
- 2026-06-01: TPR-050 implemented scoped Mermaid `themeCSS` handling in the SVG output path.
  Evidence:
  - `cargo fmt` passed.
  - `cargo nextest run -p merman-render scoped_css --no-default-features` passed: 7 tests run,
    7 passed.
  - `cargo nextest run -p merman --features render svg_pipeline_tests` passed: 4 tests run,
    4 passed.
  - `cargo nextest run -p merman-render svg` passed: 102 tests run, 102 passed.
- 2026-06-01: TPR-060 single-sourced supported theme names across core, bindings, WASM,
  `@merman/web`, and the playground.
  Evidence:
  - `cargo nextest run -p merman-core theme` passed: 8 tests run, 8 passed.
  - `cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface` passed:
    1 test run, 1 passed.
  - `cargo check -p merman-wasm --target wasm32-unknown-unknown` passed.
  - `npm run build --prefix platforms/web` passed.
  - `npm run build --prefix playground` passed, including the dist WASM presence check.
- 2026-06-01: TPR-080 closed the lane with a narrowed closeout gate and follow-up split.
  Evidence:
  - `CHANGELOG.md` updated under Unreleased / Added, Changed, and Fixed.
  - Full `compare-all-svgs` theme fixture expansion, `neo/redux` themes, and remaining
    diagram-specific resolver migrations are recorded as follow-ups.

## Known Risks

- Full Mermaid theme expansion can create large SVG diffs. Use targeted tests before broad gates.
- Exact color serialization must remain stable for upstream parity.
- Remaining diagram modules still contain local theme reads where no common resolver migration was
  justified in this lane.
- `neo` and `redux` Mermaid theme families remain unsupported.
- Broad theme fixture parity is not yet a closed asset; add representative fixtures before claiming
  full theme parity across all diagram types.

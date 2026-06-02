# Theme Parity Refactor - Evidence And Gates

Status: Complete
Last updated: 2026-06-02

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

Post-11.15 theme surface hardening:

```sh
cargo nextest run -p merman-core theme
cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface
cargo nextest run -p merman-render flowchart_svg
cargo nextest run -p merman-render neutral_named_white_edge_label_background_fades_to_white unknown_edge_label_background_keeps_mermaid_default_fade
cargo nextest run -p merman --features render external_site_theme external_snapshot_only_theme
npm run build:ts --prefix platforms/web
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme
```

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
- 2026-06-02: TPR-090 reopened the lane for Mermaid 11.15 theme surface hardening.
  Finding:
  - Mermaid 11.15 config theme selection exposes `default`, `base`, `dark`, `forest`, and
    `neutral`. Snapshot-only `neo/redux*` variable files are not official config theme names.
  - Playground compare mode diverged when Merman exposed `neo/redux*`: Mermaid fell back to
    default while Merman applied snapshot-only variables.
  - Flowchart neutral `.labelBkg` used `rgba(232, 232, 232, 0.5)` because CSS color fading did not
    parse the named color `white`; Mermaid emits white for neutral `edgeLabelBackground`.
  Evidence:
  - `cargo nextest run -p merman-core theme` passed: 10 tests run, 10 passed.
  - `cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface`
    passed: 1 test run, 1 passed.
  - `cargo nextest run -p merman-render flowchart_svg` passed: 11 tests run, 11 passed.
  - `npm run build:ts --prefix platforms/web` passed.
  - `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_docs_directives_changing_theme_via_directive_009 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/theme-diagnose/flowchart-theme-forest-after.md`
    passed.
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme --diagram flowchart --diagram xychart --diagram gitgraph --diagram pie --diagram gantt --diagram architecture --diagram quadrantchart --diagram class --diagram sequence --diagram radar --diagram er --diagram timeline --diagram packet --diagram treemap`
    passed.
  Residual risk:
  - A broader `cargo nextest run -p merman-render flowchart` run hit an existing browser/Katex
    measurement assertion unrelated to the theme surface change:
    `math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell`,
    `matrix width = 259.390625`.
- 2026-06-02: TPR-100 added representative ordinary-source/external-theme coverage without adding
  frontend test infrastructure.
  Evidence:
  - `cargo nextest run -p merman-render neutral_named_white_edge_label_background_fades_to_white unknown_edge_label_background_keeps_mermaid_default_fade`
    passed: 2 tests run, 2 passed.
  - `cargo nextest run -p merman --features render external_site_theme external_snapshot_only_theme`
    passed: 2 tests run, 2 passed.
  - `cargo nextest run -p merman-render flowchart_svg` passed: 11 tests run, 11 passed.
  - `npm run build:ts --prefix platforms/web` passed.
  Notes:
  - The `merman` high-level tests use `HeadlessRenderer::with_site_config` to model a playground
    theme selector on plain source, rather than embedding a Mermaid directive in the source.
  - `crates/merman` now has `serde_json` as a dev-dependency only, so tests can construct
    `MermaidConfig` values without expanding production dependencies.
  - Remaining diagram-specific resolver migration is split; no additional abstraction was forced
    without per-diagram parity evidence.

## Known Risks

- Full Mermaid theme expansion can create large SVG diffs. Use targeted tests before broad gates.
- Exact color serialization must remain stable for upstream parity.
- Remaining diagram modules still contain local theme reads where no common resolver migration was
  justified in this lane.
- Snapshot-only `neo/redux*` theme variable files remain unsupported as config theme names.
- Broad theme fixture parity is not yet a closed asset; add representative fixtures before claiming
  full theme parity across all diagram types.

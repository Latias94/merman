# HPD-080 Resvg-Safe All-Supported Audit And Treemap ClassDef

Date: 2026-06-02

## Scope

- Resolved the Flowchart `layout.rs` conflict between the Zed PR 58325 stack-overflow fix and the
  later local explicit-stack cluster traversal coverage.
- Re-audited Zed PR 57967 theme feedback against the current 0.7 theme/pipeline surface.
- Ran the manual ignored all-supported fixture audit for `HeadlessRenderer::render_svg_resvg_safe_sync`.

## Findings

- Common host theme needs are covered by the current API surface:
  - Mermaid theme and `themeVariables` via `site_config` / init directives.
  - Host palette CSS via `ScopedCssPostprocessor` and binding `svg.scoped_css`.
  - Optional `!important` cleanup via `CssOverridePolicy::StripExistingImportant`.
  - Root canvas replacement via `RootBackgroundPostprocessor` / `svg.root_background_color`.
  - Duplicate native/fallback label cleanup via `DropNativeDuplicateFallbacksPostprocessor` /
    `svg.drop_native_duplicate_fallbacks`.
- Zed's exact editor palette cleanup remains host policy. The useful generic contract is fallback
  markers plus optional duplicate-fallback cleanup.
- The manual all-supported audit found one real supported-family defect after skipping parser-only
  or invalid upstream-doc fixtures: Treemap rejected Mermaid-compatible bare classDef label-style
  tokens such as `color`.

## Changes

- `flowchart/layout.rs` conflict resolution keeps explicit-stack traversal coverage and the
  `MAX_DIAGRAM_NESTING_DEPTH` model guard.
- Empty Pie roots now emit a finite `viewBox="0 0 450 450"` for headless/raster safety instead of
  copying Mermaid's invalid `-Infinity` capture artifact.
- Treemap `classDef` validation now accepts bare label-style tokens that Mermaid accepts.
- Treemap SVG style compilation skips empty-valued declarations, so accepted tokens like `color`
  do not leak `color: !important` or `fill: !important` into headless output.
- The ignored all-supported resvg-safe audit now skips known parser-only/invalid fixtures and passes
  over the supported fixture set.

## Verification

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack extract_descendants_handles_deeply_nested_subgraphs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks svg_options_can_inject_host_scoped_css svg_options_can_set_root_background_color`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render pie --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core treemap_classdef_allows_bare_label_style_tokens_like_mermaid`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render treemap --test treemap_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`
- `git diff --check`

## Follow-Up

- Keep using the ignored all-supported audit as a manual triage tool, not as a percentage-style
  parity claim.
- Continue HPD-080 with visible rendering defects: blank output, hidden text, black blocks, lost
  semantic colors, crashes, and parsed-but-not-emitted style/config options.

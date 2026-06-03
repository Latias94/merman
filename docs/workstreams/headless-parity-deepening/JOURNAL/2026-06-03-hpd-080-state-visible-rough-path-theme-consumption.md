# HPD-080 - State Visible Rough-Path Theme Consumption

Task: HPD-080 visible rendering defect triage.

## Question

Does current State output actually consume Mermaid 11.15 theme values on the visible DOM, or does
it only emit correct CSS tokens while rough inline shapes keep stale hardcoded colors?

## Source Audit

- Pinned Mermaid 11.15 `state/styles.js` themes visible State surfaces through `.node rect`,
  `.node polygon`, `.node .fork-join`, `.node circle.state-end`, and `.statediagram-note rect`.
- Current local State output renders many of those same visible surfaces as rough inline `<path>`
  pairs instead: ordinary State nodes, `choice`, `fork` / `join`, `stateEnd`, and notes.
- The local stylesheet already emitted the expected Mermaid theme tokens, but those visible rough
  shapes still carried hardcoded inline fill/stroke/stroke-width defaults, so stylesheet parity
  alone did not recolor the current DOM.

## Outcome

- Added `StateThemeDefaults` in `crates/merman-render/src/svg/parity/state/style.rs` to parse the
  State-relevant Mermaid defaults from `effective_config`.
- Threaded those defaults through `StateRenderCtx` and used them only at final SVG attribute
  emission for ordinary State, `choice`, `fork` / `join`, `stateEnd`, and note rough output.
- Left rough caches geometry-only: cached `d` values and cache keys still depend only on shape
  geometry and seed, not on theme colors.
- Preserved default baseline behavior by keeping the existing rough fallback stroke width at `1.3`
  when Mermaid's `strokeWidth` remains the default `1`.
- Preserved explicit override precedence by keeping `style` / `classDef` declarations on the
  emitted paths as `!important` style attributes instead of folding those overrides into cache or
  default theme state.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render --test state_svg_test state_svg_honors_theme_options_on_visible_rough_paths` -
  passed, `1` test run.
- `cargo nextest run -p merman-render --test state_svg_test` - passed, `3` tests run.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\state_report_parity_after_hpd080_state_inline_theme.md` -
  passed, all fixtures matched.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\state_report_parity_root_after_hpd080_state_inline_theme.md` -
  passed, no structural root regression.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `git diff --check` - passed.

## Residual

- Public dark-theme smoke still checks that State theme signals remain present through the public
  render API, but the honest regression gate for this seam is the focused State renderer test over
  the final visible rough-path attributes.
- Neo gradient/drop-shadow and dependency-marker rules remain deferred until current local State
  output emits the required support DOM.

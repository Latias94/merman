# HPD-080 - C4 Visible Signal Boundary

Task: HPD-080 visible rendering defect triage.

## Question

Does C4 still have a user-visible theme/renderability gap after the recent hardcoded-color and
visible-signal audits?

## Source Audit

- Pinned Mermaid 11.15 `c4/styles.js` emits only `.person` CSS from `personBorder` and
  `personBkg`.
- Pinned `c4/svgDraw.js` renders current C4 shapes under `class="person-man"` and writes visible
  fill/stroke values inline from `c4.*_bg_color`, `c4.*_border_color`, or per-shape style macros.
- `svgDrawCommon.drawRect(...)` only emits a `class` attribute when the caller supplies one; C4
  shape rects do not supply `class="person"`.

## Outcome

No production renderer defect was found. The risk was smoke-test honesty: `themeVariables.personBkg`
and `personBorder` can appear in source-backed provider CSS without styling current C4 DOM.

Added `c4_theme_smoke_counts_inline_config_and_style_macros_as_visible` in
`crates/merman/tests/theme_renderability_smoke.rs`. It proves:

- `.person` provider CSS is still emitted;
- current output exposes `person-man` groups, not `class="person"`;
- `c4` config colors reach visible system shapes;
- `UpdateElementStyle(...)` colors reach visible shape and label output;
- `UpdateRelStyle(...)` colors reach visible relationship line and label output.

Updated the HPD-080 coverage ledger to record C4 as covered through inline config/style macros, with
the `.person` provider rule tracked as provider-only unless upstream DOM changes.

## Verification

- `cargo nextest run -p merman --features render --test theme_renderability_smoke c4_theme_smoke_counts_inline_config_and_style_macros_as_visible`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt`

## Residual

Do not promote generic `themeVariables.personBkg` / `personBorder` into visible C4 shape colors
without new source evidence. C4's current visible color API remains inline `c4` config and C4 style
macros.

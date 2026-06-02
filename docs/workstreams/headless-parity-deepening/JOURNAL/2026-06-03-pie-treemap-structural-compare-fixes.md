# 2026-06-03 - Pie/Treemap Structural Compare Fixes

Scope: HPD-080 visible rendering defect triage.

User report:

- CI showed stale failures for Sequence default message width (`161` vs `160`) and Class namespace
  semantic snapshots on Windows, macOS, and Linux.

Diagnosis:

- Current HEAD no longer contains the old Sequence test name; the calibrated
  `sequence_default_message_widths_use_current_sequence_svg_bbox_facts` passes.
- Current HEAD semantic snapshots pass, including the Class namespace facade update.
- Fresh all-diagram SVG DOM compare exposed two real current defects instead:
  - Pie fixtures with unrelated `themeVariables` overrides used local HSL-rewritten `pie1` instead
    of Mermaid 11.15's raw `#ECECFF`.
  - Treemap `classDef ... color;` rendered locally as a treemap, while pinned upstream renders the
    fixture as an error diagram.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/theme-default.js` assigns `pie1` from
  `primaryColor` and `pie2` from `secondaryColor` without serializing those base colors through
  HSL.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/db.ts` has tolerant `addClass` style
  splitting, but the parity contract for the fixture is the parser/render result. The pinned
  upstream SVG baseline is an `aria-roledescription="error"` diagram.

Changes:

- `apply_default_theme_defaults` now preserves raw `primaryColor` / `secondaryColor` strings for
  default-theme `pie1` / `pie2` when user `themeVariables` trigger merge mode.
- Treemap classDef validation now rejects bare style tokens that lack `:`, so suppress-errors parse
  produces the Mermaid-compatible error diagram for the invalid fixture.
- Updated focused Pie layout snapshots and Treemap semantic/layout snapshots.
- Replaced the old Treemap render test that assumed parser acceptance with an end-to-end
  suppress-errors error-SVG assertion.

Verification:

- `cargo fmt`
- `cargo nextest run -p merman-render sequence_default_message_widths_use_current_sequence_svg_bbox_facts`
- `cargo nextest run -p merman-core --test snapshots fixtures_match_golden_snapshots`
- `cargo nextest run -p merman-core default_theme_merges_unrelated_theme_variable_overrides_without_hsl_rewriting_pie_base default_theme_preserves_user_overrides_after_derivation supported_theme_defaults_match_upstream_snapshot`
- `cargo nextest run -p merman-core treemap_classdef_rejects_bare_label_style_tokens_like_mermaid_parser`
- `cargo nextest run -p merman-render --test treemap_svg_test treemap_classdef_bare_label_style_token_renders_error_like_mermaid_parser`
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run --workspace --all-features`

Result:

- Full SVG structural parity is green.
- Workspace all-features nextest passed `1681/1681`, with `6` skipped.

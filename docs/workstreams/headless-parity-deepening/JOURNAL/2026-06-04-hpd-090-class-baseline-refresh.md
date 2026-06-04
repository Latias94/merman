# HPD-090 Class Baseline Refresh

Class was the first narrow stale stored-SVG set after the broad stale family queue was cleared.

Outcome:

- Point-refreshed the two stale Class upstream SVG baselines:
  `stress_class_svg_font_size_px_string_precedence_026` and `upstream_parser_class_spec`.
- DOM parity was not a pure fixture refresh. Mermaid 11.15's native SVG-label path keeps the
  `: String` type suffix on the second outer `tspan` for the `htmlLabels=false` px-string
  font-size probe, while local output previously wrapped it onto a third line.
- Updated Class layout and SVG-render wrapping heuristics for the source-backed case where
  `calculateTextWidth(...)` uses a smaller top-level `fontSize` probe but the final SVG text
  inherits a larger explicit `themeVariables.fontSize` px value.
- Added focused coverage so the native SVG wrapping keeps the type suffix in the same outer
  `tspan` instead of regressing to a standalone third `String` row.
- Refreshed the affected `stress_class_svg_font_size_px_string_precedence_026` layout golden and
  added the missing existing-fixture `zed_pr_57644_class.layout.golden.json` snapshot.

Verification:

- `cargo nextest run -p merman-render --test class_svg_test class_svg_px_string_theme_font_size_uses_mermaid_svg_label_wrapping` -
  passed, `1` test run.
- `cargo nextest run -p merman-render --test class_svg_test` - passed, `21` tests run.
- `cargo run -p xtask -- update-layout-snapshots --diagram class` - passed and produced the Class
  layout golden updates above.
- `cargo nextest run -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present` -
  passed, `1` test run.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\class_report_parity_hpd090_after_wrap_fix.md` -
  passed.
- `cargo fmt -p merman-render --check` - passed.

Residual note:

- Class structural DOM parity is green after the narrow refresh. Remaining HPD-090 narrow work is
  `timeline` (`1` fixture) and Flowchart HTML demo KaTeX drift (`4` fixtures), followed by the
  readiness gates.

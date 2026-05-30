# Flowchart Text Style Parity Evidence And Gates

## Required Gates

- `cargo fmt --check`
- `cargo nextest run -p merman-render --test flowchart_svg_test --test flowchart_layout_test`

## Later Gates

- `cargo run -p xtask -- compare-all-svgs --diagram flowchart`
- `cargo run -p xtask -- compare-all-svgs --diagram flowchart --mode parity-root`
- `cargo run -p xtask -- verify --strict`

## Evidence Log

- 2026-05-30: `cargo fmt --check` passed.
- 2026-05-30: `cargo nextest run -p merman-render --test flowchart_svg_test --test flowchart_layout_test` passed, 32 tests.
- 2026-05-30: `cargo nextest run -p merman-render --test er_svg_test` passed, 5 tests. This guards the shared style-helper expansion for an adjacent consumer.
- 2026-05-30: Attempted `cargo nextest run -p merman-render --test block_svg_test --test er_svg_test`; no `block_svg_test` target exists in `merman-render`.
- 2026-05-30: `cargo fmt --check` passed after TSP-030.
- 2026-05-30: `cargo nextest run -p merman-render --test flowchart_layout_test --test flowchart_svg_test` passed, 33 tests. Added coverage: `flowchart_whole_label_font_style_italic_affects_node_label_layout`.
- 2026-05-30: `cargo nextest run -p merman-render --lib` compiled and ran 143 lib tests; 142 passed, 1 failed in `math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` because the local Node/KaTeX browser-shell probe measured node height as `27.265625`, below that test's external-environment assertion range.
- 2026-05-30: `cargo nextest run -p merman-render --lib math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` reproduced the same external browser-shell height failure.
- 2026-05-30: `cargo nextest run -p merman-render --lib -- --skip math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` passed, 142 tests, 1 skipped.

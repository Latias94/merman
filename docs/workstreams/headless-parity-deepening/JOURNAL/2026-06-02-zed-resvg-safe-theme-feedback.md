# HPD-080 - Zed Resvg-Safe Theme Feedback

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## External Signal

- Zed PR: <https://github.com/zed-industries/zed/pull/57967>
- Reviewer color feedback:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939>
- Zed follow-up comment:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388>
- Zed `color cleanup` commit:
  `c85f29cd2e78ec8a68b20349606d8298eecf37bb`

## Finding

- Zed's color cleanup is mostly host theme policy: background replacement, injected edge-label
  colors, and tag-label fill are Zed-specific attempts to preserve its current markdown preview
  appearance after adopting merman's `resvg_safe` pipeline.
- The fallback cleanup change is more general. Zed previously dropped fallback overlay groups when
  any native SVG text existed; this can remove fallback-only labels. Their fix keeps fallback labels
  unless the fallback text duplicates a native SVG `<text>` label.
- merman already marks generated fallback groups with `data-merman-foreignobject="fallback"` and
  fallback text with `merman-foreignobject-fallback-text`, so this is a clean optional pipeline
  contract.

## Change

- Added `DropNativeDuplicateFallbacksPostprocessor`.
- Exported it through `merman_render::svg` and `merman::render`.
- The default `SvgPipeline::resvg_safe()` behavior is unchanged. Consumers can opt in with:
  `SvgPipeline::resvg_safe().with_postprocessor(DropNativeDuplicateFallbacksPostprocessor)`.

## Verification

- `cargo fmt -p merman-render -p merman`
- `cargo fmt --check -p merman-render -p merman`
- `cargo test -p merman-render drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render resvg_safe_can_optionally_drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render svg::pipeline --lib`
- `cargo test -p merman-render foreign_object --lib`
- `git diff --check`

## Negative Gate

- `cargo test -p merman-render --lib` is not green at current HEAD because of existing
  measurement-sensitive tests outside this change:
  - `sequence_default_message_widths_match_mermaid_default_font_family`: `161.0` vs `160.0`
  - `node_katex_math_renderer_measures_sanitized_flowchart_browser_shell`: matrix width
    `282.265625`

## Residual

- Host palette injection remains a consumer concern. We should not turn Zed's theme CSS cleanup into
  merman default output.
- The fallback de-duplication pass is text-based. It intentionally removes only exact normalized
  text duplicates and leaves geometry or style equivalence decisions to consumers.

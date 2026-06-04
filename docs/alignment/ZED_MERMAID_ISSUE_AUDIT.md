# Zed Mermaid Issue Audit

Date: 2026-05-28
Updated: 2026-06-02

This audit maps Mermaid-related Zed issues and PRs to merman behavior. It focuses on the Zed
migration from `mermaid-rs-renderer` to `merman` in zed-industries/zed#57644, plus the issue shapes
that are useful as headless regression fixtures.

## Integration Shape

The audited Zed tree did not fork `merman` source. It had an internal `crates/mermaid_render`
wrapper that depended on `merman = { version = "0.4", features = ["render"] }` and then applied
Zed-specific theme/accent/resvg post-processing. Later PR zed-industries/zed#57967 updates that
dependency to `0.6` and adopts `SvgPipeline::resvg_safe()`. Treat both versions of the wrapper as
requirements evidence, not as code to copy into this repository.

## Coverage Map

| Zed issue / PR | Issue class | merman status | Evidence |
| --- | --- | --- | --- |
| zed-industries/zed#57389 | `sequenceDiagram` loop `end` stops rendering | Covered | `fixtures/zed_issues/zed_57389_sequence_loop_end.mmd`; renders with loop label and post-loop message. |
| zed-industries/zed#57363 | Flowchart edge labels with hyphenated text parse/layout poorly | Covered for parsing/headless output | `fixtures/zed_issues/zed_57363_flowchart_hyphen_edge_labels.mmd`; resvg-safe SVG preserves the full label text. Pixel layout is not asserted here. |
| zed-industries/zed#57323 | ER entity styles show as visible CSS text | Covered for headless output | `fixtures/zed_issues/zed_57323_er_entity_style_text.mmd`; entity names/attributes render as labels. This also exposed and fixed bare `undefined` style declarations in resvg-safe output. |
| zed-industries/zed#56767 | SVG preview does not render Mermaid `<foreignObject>` labels | Covered for merman-generated output | `SvgPipeline::resvg_safe()` strips `<foreignObject>` after inserting text fallback groups. Zed SVG preview of arbitrary external Mermaid SVG remains a host-side problem unless it runs a fallback pass. |
| zed-industries/zed#51142 | Sequence `rect rgb(...)` rendered as text; repro uses `participant AS as AppService` | Fixed and covered | `fixtures/zed_issues/zed_51142_sequence_rect_rgb.mmd`; parser now handles keyword-like actor ids such as `AS`, `END`, `RECT`, and `loop`, and rect fill is emitted as a background rectangle. |
| zed-industries/zed#51480 | Larger flowchart edge rendering breaks down | Smoke covered | `fixtures/zed_issues/zed_51480_complex_flowchart_connections.mmd`; headless resvg-safe render completes and keeps labels. Edge routing is not pixel-golden in this test. |
| zed-industries/zed#50243 | Gantt `displayMode: compact` frontmatter | Already broadly covered, plus Zed fixture | Existing upstream Gantt compact fixtures remain; `fixtures/zed_issues/zed_50243_gantt_compact_frontmatter.mmd` is included in the Zed smoke. |
| zed-industries/zed#50558 / #50238 / #50485 | Class inheritance, stereotypes, dotted lines, earlier mermaid-rs fixes | Covered by existing class corpus; Zed smoke added for inheritance | `fixtures/zed_issues/zed_50558_class_inheritance.mmd` renders headless. |
| zed-industries/zed#56199 / #50176 / #50470 / #50280 | Old renderer panics on partial shapes / empty subgraphs / hex parsing | Covered at Result boundary | `fixtures/zed_issues/zed_56199_flowchart_partial_parallelogram.mmd` is rendered with lenient parsing as an error SVG without panicking. Empty subgraph coverage already exists in flowchart tests. |
| zed-industries/zed#57967 | Upgrade to `merman = "0.6"` and `SvgPipeline::resvg_safe()` changes preview colors/fallback overlays | Host theme boundary plus optional merman pipeline support | Zed's color cleanup is host palette policy. The generic fallback duplicate issue is covered by `DropNativeDuplicateFallbacksPostprocessor`, which removes only fallback groups whose text duplicates native SVG text. See `docs/workstreams/headless-parity-deepening/JOURNAL/2026-06-02-zed-resvg-safe-theme-feedback.md`. |
| zed-industries/zed#57875 | `markdown_preview_theme` not reflected in Mermaid preview | Host integration boundary | `merman` exposes `MermaidConfig`, `SvgPipeline`, and host CSS postprocessors, but Zed must pass the preview theme instead of the editor theme. |
| zed-industries/zed#56914 / #51466 / #51623 / #56695 | Fonts missing or substituted incorrectly in Zed/GPUI rasterization | Mostly host integration boundary | `merman` can use vendored measurement and host-provided font families. Actual glyph rasterization and font fallback are handled by the host SVG/raster stack. |
| zed-industries/zed#56466 / #56468 / #51242 | Huge Mermaid diagrams can allocate oversized GPUI textures | Host boundary plus covered merman raster policy | Zed must still cap preview textures when it rasterizes SVGs itself. `merman` PNG/JPG helpers now expose target-aware `fit_to` sizing plus an explicit pixmap budget, with a default `8192px` side / `8192*8192` pixel cap for untrusted oversized diagrams. |

## Practical Conclusion

`merman` should solve most old `mermaid-rs-renderer` parser/rendering failures that were fixed by
Zed's migration, especially sequence blocks, class relationships, Gantt frontmatter, flowchart label
parsing, ER labels, and panic containment. The remaining open Zed issues are mostly integration
surface:

- theme selection must be wired from the host preview theme,
- host-specific palette replacement should stay in the host or an explicit postprocessor,
- font fallback and glyph rasterization live in the host SVG renderer,
- huge texture allocation needs a host-side cap when the host rasterizes SVG itself; merman-owned
  PNG/JPG rasterization now has a reusable sizing policy,
- arbitrary external Mermaid SVGs need host SVG preview support or an explicit fallback pass.

The regression suite added in `crates/merman/tests/zed_mermaid_issue_fixtures.rs` is intentionally
not a screenshot-golden suite. It verifies the properties a Rust host needs first: parsing does not
panic, headless render returns SVG, resvg-safe output removes known raster hazards, and human labels
survive without relying on `<foreignObject>`.

# Zed Mermaid Issue Audit

Date: 2026-05-28
Updated: 2026-08-24

This audit maps Mermaid-related Zed issues and PRs to merman behavior. It focuses on the Zed
migration from `mermaid-rs-renderer` to `merman` in zed-industries/zed#57644, plus the issue shapes
that are useful as headless regression fixtures.

## Integration Shape

Older audited Zed trees did not vendor `merman` source. They had an internal
`crates/mermaid_render` wrapper that depended on released `merman` versions and then applied
Zed-specific theme/accent/resvg post-processing. Later PR zed-industries/zed#57967 updated that
dependency to `0.6` and adopted `SvgPipeline::resvg_safe()`.

The current `repo-ref/zed` checkout is `d9ad6aff67e47de43abb270d22de75dd950f1b48`.
Its `Cargo.toml` and `Cargo.lock` pin `merman = "=0.8.0-alpha.5"` from crates.io, rather than
the earlier patched `0.6.2` Git snapshot. The audited Zed checkout still calls the Merman public
API shape below. This is current integration evidence for that checkout, not an API contract that
new hosts should copy without checking the current Merman source:

- `merman::MermaidConfig::from_value(...)` for host theme variables,
- `merman::render::HeadlessRenderer::new().with_site_config(...).with_vendored_text_measurer().with_diagram_id(...)`,
- `SvgPipeline::resvg_safe().with_postprocessor(CssOverridePostprocessor::strip_existing_important())`,
- `HeadlessRenderer::render_svg_with_pipeline_sync(...)`.

The source evidence was refreshed against `repo-ref/zed/crates/mermaid_render/src/render.rs`,
`postprocess.rs`, and `postprocess/inject_css.rs`; the latter still contains the downstream
fallback typography rule at line 377.

It then performs Zed-specific `strip_foreignobject`, element fixup, accent assignment, and CSS
injection after Merman has completed terminal validation. That ordering invalidates the final
resvg-safe evidence. Zed also hands the resulting string to GPUI's shared default `usvg::Options`;
the default image string resolver treats `<image href>` values as host file paths. Merman's own
PNG/JPG/PDF exporters disable that resolver, but the protection does not follow a plain SVG string
into GPUI. On upgrade, Zed should:

- use `Presentation::with_theme(HostTheme::new()...)` or the equivalent `MermaidConfig` for theme variables,
- replace its duplicate fallback pass with
  `SvgPipeline::with_drop_native_duplicate_fallbacks(true)`,
- keep only product-specific accent assignment and palette generation in Zed,
- register those remaining transformations as `SvgPostprocessor` passes before the terminal preset,
- use `ScopedCssPostprocessor` for selector scoping and CSS insertion, and
- migrate the wrapper to `Renderer + RenderRequest::svg(...)`, attach a caller-owned
  `OperationControl`, and select the final SVG pipeline through `SvgRequest`; the public
  Headless facade and synchronous method matrix no longer exist in the current Merman source.

Current Merman additionally closes non-navigation rendering resources in the resvg-safe terminal
stage, which prevents Flowchart image paths from reaching GPUI. Zed should still use a
Mermaid-specific `usvg::Options` with `ImageHrefResolver::resolve_string` disabled as independent
defense in depth; changing the shared GPUI resolver would alter unrelated SVG product behavior.
Because GPUI rasterizes the SVG directly, that Mermaid-specific path must also bound and inspect
inline PNG/JPEG/GIF/WebP data URLs before decode. `ResvgCompatibleSvg` proves that resource
locations are closed, not that an embedded image has a cheap decoded representation.

`crates/merman/tests/zed_editor_contract.rs` covers this target integration shape inside merman:
configured SVG ids, host theme config, fallback text, `foreignObject`
removal, generic class text escaping, unsafe CSS stripping, invalid visual-attribute cleanup, and
representative Zed preview diagram families. Treat Zed wrapper code as requirements evidence, not
as code to copy into this repository.

## Recent Issue Signal

### Issue #89: fallback typography context

Issue #89 is owned by Merman's generic `foreignObject` adapter, not by a Zed-only font-size
workaround. `SvgPipeline::resvg_safe()` now resolves typography against the original SVG/XHTML
ancestry before stripping HTML, so ClassDiagram class labels retain 16px while real contextual
selectors such as ER relationship labels can retain their distinct 14px size. The Merman result is
verified before Zed postprocessing and remains parseable by the pinned workspace `usvg`.

The Zed ordering remains important: `strip_existing_important()` runs in the Merman pipeline, while
Zed's theme CSS is injected after `render_mermaid()` returns. Zed's current global
`.merman-foreignobject-fallback-text { font-size: 16px !important; }` rule is therefore a downstream
metric override. It is intentionally not changed by this issue; Zed should remove that workaround
in a later consumer change if it wants to preserve user-defined or Venn-specific non-16px metrics.
The stable fallback marker/class hooks remain available for that migration.

The newest public Zed issues and competing fixes were reviewed newest first. Open status does not
mean the defect belongs in Merman: several reports are caused by Zed's detector, family allowlist,
font database, or direct GPUI raster path.

| Date | Zed issue / PR | Current boundary | Merman evidence or required action |
| --- | --- | --- | --- |
| 2026-08-07 | [zed#62330](https://github.com/zed-industries/zed/issues/62330) (open issue at audit time) | Host lifecycle plus renderer cancellation boundary | The report shows an obsolete synchronous render can continue consuming CPU after its Markdown/ACP content is no longer useful. Merman now carries one caller-owned `OperationControl` through controlled parsing, SVG/ASCII layout and emission, postprocessing, and export. Zed should retain the control beside each cached diagram, cancel it on cache eviction/content replacement/archive, and reject stale completion with a generation ID. `max_layout_work_units` remains a deterministic resource ceiling; it does not replace cancellation or a deadline. |
| 2026-07-26 | [zed#61678](https://github.com/zed-industries/zed/pull/61678) (open PR) | Incomplete host-side fix | It handles YAML frontmatter only and deliberately leaves the reported leading `%%{init}%%` case unresolved. Prefer detector delegation in zed#61644 so Zed cannot drift from Merman's preprocessor. |
| 2026-07-25 | [zed#61644](https://github.com/zed-industries/zed/pull/61644) (open PR) | Correct host detector fix | Delegating type detection to Merman covers frontmatter, directives, and leading comments with the same preprocessing path used by rendering. This is preferable to copying another preamble parser into Zed. |
| 2026-07-25 | [zed#61617](https://github.com/zed-industries/zed/issues/61617) (open issue) | Fixed in Zed's pinned Merman | Merman PR #29 replaced unsafe byte-offset Gantt date slicing and added Japanese/full-width regressions. The audited `0.8.0-alpha.5` dependency contains merge commit `95943d899c830cb15ab8f7f2b7be8a39d94b2006`; any remaining failure needs reproduction in the current Zed host path. |
| 2026-07-24 | [zed#61586](https://github.com/zed-industries/zed/issues/61586) (open issue) | Host font-rasterization boundary | Merman can measure with vendored or host-provided fonts, but GPUI/usvg selects the actual glyph fallback used for pixels. Zed must make its Mermaid font database and fallback selection cover the document's scripts. |
| 2026-07-20 | [zed#61361](https://github.com/zed-industries/zed/issues/61361) (open issue) | Zed first-token detector bug | Merman already preprocesses frontmatter, directives, and comments before detection. Zed should call that detector, as zed#61644 does, rather than reject the source before Merman sees it. |
| 2026-07-02 | [zed#60272](https://github.com/zed-industries/zed/issues/60272) (open issue) | Intentional Zed family-admission boundary | Merman parses and renders C4 with a dedicated fixture corpus. Zed explicitly excludes C4 until its host theme CSS makes text readable; this is not a missing Merman family. |
| 2026-06-21 | [zed#59651](https://github.com/zed-industries/zed/issues/59651) (open issue) | Fixed in the current Merman worktree | Sequence layout now records canonical block start, stop, and section coordinates. SVG consumes those facts instead of reconstructing vertical geometry with fixed offsets; the issue's nested note/loop/alt source is a structural regression test. |
| 2026-06-05 | [zed#58707](https://github.com/zed-industries/zed/issues/58707) (open issue) | Covered by current Merman | Class relation cardinality terminals are emitted with measured positive bounds in normal and hand-drawn output. The audited Zed dependency is newer than the originally reported preview; reproduce against the current checkout before assigning any residual to Merman. |

## Historical Coverage Map

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
| zed-industries/zed#57967 | Upgrade to `merman = "0.6"` and `SvgPipeline::resvg_safe()` changes preview colors/fallback overlays | Host theme boundary plus current merman pipeline support | Zed's color cleanup remains host palette policy. Generic duplicate fallback cleanup is now `SvgPipeline::with_drop_native_duplicate_fallbacks(true)`. Zed-specific passes must run before terminal finalization rather than rewriting the sealed output string. |
| zed-industries/zed#57875 | `markdown_preview_theme` not reflected in Mermaid preview | Host integration boundary | `merman` exposes `MermaidConfig`, `SvgPipeline`, and host CSS postprocessors, but Zed must pass the preview theme instead of the editor theme. |
| zed-industries/zed#56914 / #51466 / #51623 / #56695 | Fonts missing or substituted incorrectly in Zed/GPUI rasterization | Mostly host integration boundary | `merman` can use vendored measurement and host-provided font families. Actual glyph rasterization and font fallback are handled by the host SVG/raster stack. |
| zed-industries/zed#56466 / #56468 / #51242 | Huge Mermaid diagrams can allocate oversized GPUI textures | Host boundary plus covered merman raster policy | Zed must still cap preview textures when it rasterizes SVGs itself. `merman` PNG/JPG helpers now expose target-aware `fit_to` sizing plus an explicit pixmap budget, with a default `4096px` side / `4096*4096` pixel cap for untrusted oversized diagrams. |

## Practical Conclusion

`merman` should solve most old `mermaid-rs-renderer` parser/rendering failures that were fixed by
Zed's migration, especially sequence blocks, class relationships, Gantt frontmatter, flowchart label
parsing, ER labels, and panic containment. Recent issues reinforce that Zed should keep its released
`0.8.0-alpha.5` dependency current and keep product admission separate from parser detection. The
remaining open Zed issues are mostly integration surface:

- detection should delegate to Merman before Zed applies its supported-family allowlist,
- each cached render should own an `OperationControl`; content replacement, eviction, archive, or
  close should cancel the previous control before starting a replacement,
- a monotonic deadline should bound one synchronous render, while a generation ID prevents a
  completion that raced with cancellation from replacing newer output,
- C4 and other deliberately blocked families need Zed theme/admission work rather than another
  Merman parser,
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

Cancellation is cooperative rather than a safe thread kill. Merman checkpoints CPU work it owns,
but an opaque host callback or monolithic third-party encoder may return before the next checkpoint.
Hosts that require hard preemption must isolate that work in a worker or process they can terminate.

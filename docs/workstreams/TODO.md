# Workstreams TODO (Alignment / Parity Hardening)

This TODO list is a **work-in-progress execution plan** for closing the most common Mermaid parity
gaps. Each item includes a lightweight “gap check” so we don’t fix problems we don’t have.

Baseline upstream: Mermaid `@11.12.3`.

## How to use this file

For each item:

1. Prove the gap exists (or mark it as “Already covered” with evidence).
2. If the gap exists, add the smallest reproducer fixture.
3. Fix in the model (preferred) or via a scoped override (when justified).
4. Add regression coverage.

## Global gap checks (run first)

- [x] Confirm current parity gates are green:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
  - `cargo run -p xtask -- check-alignment`
- [x] Inventory override footprint (to avoid unbounded growth):
  - `cargo run -p xtask -- report-overrides`

## A) Text measurement & wrapping

- [x] URL / punctuation line breaking in HTML labels  
  Gap check:
  - Search for failing strict-XML cases in local stress fixtures (if any).
  - Add a minimal flowchart fixture with a URL in parentheses at `wrappingWidth=200`.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3`
  - Strict stress (optional): `compare-svg-xml --dom-mode strict --dom-decimals 3`
  Notes:
  - Fixture: `fixtures/flowchart/stress_flowchart_html_label_url_punct_wrap_067.mmd`

- [x] Mixed CJK + ASCII + emoji measurement stability  
  Gap check:
  - Look for existing fixtures: `rg -n "中文|漢字|emoji|😀" fixtures/ docs/alignment -S`
  - If present, verify parity-root stability at `--dom-decimals 3`.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_unicode_punct_in_ids_labels_035.mmd`
  - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter unicode_punct_in_ids_labels_035`

- [x] Whitespace corner cases (`&nbsp;`, multiple spaces, trailing spaces)  
  Gap check:
  - Find tests: `rg -n "nbsp|multiple spaces|trailing" crates/merman-render/src/text -S`
  - Add a fixture that includes repeated spaces and verify DOM parity does not regress.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_html_label_whitespace_068.mmd`
  - Compare (structure): `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter whitespace_068`

- [x] HTML label whitespace stress fixture (`&nbsp;`, multiple spaces, trailing spaces)  
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_html_label_whitespace_068.mmd`
  - Compare (structure): `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter whitespace_068`
  - Compare (root viewport): `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter whitespace_068`

- [x] Quoted-string whitespace height parity (leading+trailing vs trailing-only)  
  Gap check:
  - Confirm we do not inflate node height for trailing-only whitespace in `labelType=string` labels.
  Evidence:
  - `parity-root` should match upstream: `--dom-mode parity-root --dom-decimals 6`.

- [x] Newline normalization (`\\n` literal vs newline vs `<br>` variants)  
  Gap check:
  - Locate existing coverage: `rg -n "replace_br_variants|<br" crates/merman-render/src/text.rs`
  - Ensure both SVG labels and HTML labels handle `<br/>` consistently.
  Evidence:
  - Implementation: `crates/merman-render/src/text.rs` (`replace_br_variants`)
  - Tests: `text::tests::html_br_trims_trailing_space_before_break_for_flowchart_labels`, `text::tests::wrap_label_like_mermaid_does_not_split_escaped_br`

## B) `htmlLabels` semantics and config precedence

- [x] Flowchart label-mode precedence matrix  
  Gap check:
  - Build a small fixture matrix:
    - global `htmlLabels` (T/F)
    - `flowchart.htmlLabels` (unset/T/F)
    - node vs edge vs subgraph titles
  Evidence:
  - Fixtures:
    - `fixtures/flowchart/stress_flowchart_html_labels_global_false_flowchart_true_069.mmd`
    - `fixtures/flowchart/stress_flowchart_html_labels_global_true_flowchart_false_070.mmd`
    - `fixtures/flowchart/stress_flowchart_html_labels_global_false_flowchart_unset_071.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter stress_flowchart_html_labels_global_`
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_html_labels_global_`

- [x] `wrappingWidth` applies to the right label categories  
  Gap check:
  - Ensure node HTML label max-width tracks `flowchart.wrappingWidth`.
  - Ensure edge labels remain capped at 200px unless upstream says otherwise.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_wrappingwidth_node_vs_edge_072.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter stress_flowchart_wrappingwidth_node_vs_edge_072`
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_wrappingwidth_node_vs_edge_072`

- [x] Class note / relation-title `htmlLabels` split  
  Gap check:
  - Ensure class notes follow global `htmlLabels` (including SVG-text mode + class-padding sizing).
  - Ensure relation title labels only flip to SVG text when `flowchart.htmlLabels=false` is explicitly active.
  - Ensure empty relation-title placeholders keep Mermaid’s HTML placeholder structure when the flowchart override is unset.
  - Status: `probe_class_htmllabels_false_note_983` / `probe_class_flowchart_htmllabels_false_edge_text_984` strict XML is now green by sizing single-line plain SVG labels from computed text length (1/64px upward quantization), mirroring Mermaid `createText()` note `bbox.y` offset in SVG mode, and passing `aria-roledescription="classDiagram"` for `classDiagram-v2` fixtures in `compare-svg-xml`.
  Evidence:
  - Fixtures:
    - `fixtures/class/probe_class_htmllabels_false_note_983.mmd`
    - `fixtures/class/probe_class_flowchart_htmllabels_false_982.mmd`
    - `fixtures/class/probe_class_flowchart_htmllabels_false_edge_text_984.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter probe_class_htmllabels_false_note_983`
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter probe_class_flowchart_htmllabels_false_982`
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter probe_class_flowchart_htmllabels_false_edge_text_984`
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter probe_class_flowchart_htmllabels_false_`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter probe_class_flowchart_htmllabels_false_ --dom-mode strict --dom-decimals 3`

- [x] Class global `htmlLabels=false` simple-node sizing / root viewport drift  
  Gap check:
  - `probe_class_htmllabels_false_981` now matches upstream for single-glyph class titles by sizing the SVG title row from Mermaid-style bold computed text length (with 1/64px upward quantization) instead of the generic SVG bbox approximation.
  Evidence:
  - Fixture: `fixtures/class/probe_class_htmllabels_false_981.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter probe_class_htmllabels_false_981`
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter probe_class_htmllabels_false_981`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter probe_class_htmllabels_false_981 --dom-mode strict --dom-decimals 3`

- [x] Class `direction` + `htmlLabels=true` empty-body / members strict parity  
  Gap check:
  - Ensure `direction LR/TB` class fixtures keep Mermaid `shapeUtil.ts` / `classBox.ts` compartment transforms.
  - Ensure HTML class labels use Mermaid-like dynamic `max-width`, relation path `style` attributes, and the fixed Rough.js seed path data used by upstream class boxes/dividers.
  - Status: exact strict-XML compares are now green for `probe_class_direction_lr_991`, `probe_class_flowchart_htmllabels_false_982`, and `probe_class_direction_lr_members_994` after switching class HTML-label `max-width` caps to a Mermaid-like hybrid width probe (Arial + configured family), while preserving Mermaid-style single-character and short-row fallback caps.
  Evidence:
  - Fixtures:
    - `fixtures/class/probe_class_direction_lr_991.mmd`
    - `fixtures/class/probe_class_direction_tb_992.mmd`
    - `fixtures/class/probe_class_direction_lr_note_993.mmd`
    - `fixtures/class/probe_class_direction_lr_members_994.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter probe_class_direction_`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter probe_class_direction_ --dom-mode strict --dom-decimals 3`

- [x] Class node `style` / `classDef` passthrough (`classBox.ts`)  
  Gap check:
  - Ensure `classNode.styles.join(';')` reaches the class box background/border paths, divider groups/paths, and HTML `span.nodeLabel` content.
  - Status: strict XML is now green for `upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_style_definition_witho_045`; styled HTML fixtures such as `upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_classdefs_being_applie_048` and `upstream_docs_classdiagram_styling_a_node_059` are reduced to residual width/layout deltas instead of missing inline style attrs.
  Evidence:
  - Fixtures:
    - `fixtures/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_style_definition_witho_045.mmd`
    - `fixtures/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_classdefs_being_applie_048.mmd`
    - `fixtures/class/upstream_docs_classdiagram_styling_a_node_059.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_style_definition_witho_045 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_classdefs_being_applie_048 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter upstream_docs_classdiagram_styling_a_node_059 --dom-mode strict --dom-decimals 3`

- [x] Class HTML-label width repeat offenders (`classBox.ts` / `shapeUtil.ts`)  
  Gap check:
  - Audit the remaining single-line HTML title/member/method rows where Mermaid browser widths differ from our hybrid probe by sub-pixel amounts and leak into strict XML via node width, divider path, and edge routing deltas.
  - Status: targeted calc/rendered width overrides now cover recurring class rows such as `Duck`, `Fish`, `Zebra`, `C1`, `Class01`, `C3`, `equals()`, `-int privateChimp`, `+int publicGorilla`, `#int protectedMarmoset`, `Object[] elementData`, `+String beakColor`, `+String gender`, `+int age`, `+mate()`, `+swim()`, `+quack()`, `-int sizeInFeet`, `-canEat()`, `+bool is_wild`, `+run()`, and the malformed Markdown/classifier rows `+inline: **bold*`, `+attribute *italic*`, `~attribute **bold**`, `_italicmethod_()`, `__boldmethod__()`, `_+_swim_() : a_`, and `__+quack() : test__`. Class HTML member/method rows now also honor classifier-derived inline row styles during HTML rendering/measurement. This makes `class/basic`, `upstream_examples_class_basic_class_inheritance_001`, `stress_class_click_and_links_011`, `stress_class_click_strict_sanitization_015`, `stress_class_font_size_precedence_024`, `stress_class_markdown_inline_code_022`, `stress_class_markdown_member_strong_023`, `upstream_cypress_classdiagram_elk_v3_spec_elk_1_1_should_render_a_simple_class_diagram_without_htmllabels_003`, `upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_markdown_styling_w_050`, `upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_markdown_styling_050`, `upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_markdown_styling_witho_050`, `upstream_docs_classdiagram_styling_a_node_059`, and the remaining style/classDef/no-members fixtures strict-green; full class strict mismatches drop from `232` to `200` with no added regressions.
  Evidence:
  - Fixtures:
    - `fixtures/class/basic.mmd`
    - `fixtures/class/upstream_examples_class_basic_class_inheritance_001.mmd`
    - `fixtures/class/stress_class_click_and_links_011.mmd`
    - `fixtures/class/stress_class_click_strict_sanitization_015.mmd`
    - `fixtures/class/stress_class_font_size_precedence_024.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter basic --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter upstream_examples_class_basic_class_inheritance_001 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --dom-mode strict --dom-decimals 3`

- [x] Class HTML note fragment / width parity (`classRenderer-v2.ts`)  
  Gap check:
  - Measure class HTML notes from the sanitized XHTML fragment that Mermaid actually injects into `foreignObject`, instead of a plain wrapped-text fallback.
  - Reuse layout-captured note label metrics during SVG emission so `foreignObject width/height` stays aligned with the layout pass.
  - Mirror Mermaid note `<div>` style ordering (`text-align` first) and fill in the remaining small repeat-offender width overrides (`Foo1`, `int id`, `size()`, plus the simple-note texts).
  - Status: class HTML notes now share a single sanitize/measure/render path, note `foreignObject` sizing stays on the layout metrics, and Mermaid-matching note `<div>` styles are emitted for both nowrap and wrapped-note cases. This makes `upstream_cypress_classdiagram_spec_19_should_render_a_simple_class_diagram_with_notes_017`, `upstream_cypress_classdiagram_v2_spec_18b_should_render_a_simple_class_diagram_with_notes_023`, `upstream_cypress_classdiagram_v3_spec_18b_should_render_a_simple_class_diagram_with_notes_031`, `upstream_cypress_classdiagram_elk_v3_spec_elk_18b_should_render_a_simple_class_diagram_with_notes_031`, and `upstream_separators_labels_notes` strict-green; note-filter mismatches fall from `7` to `2`, and full class strict mismatches drop from `200` to `196`.
  Evidence:
  - Fixtures:
    - `fixtures/class/upstream_cypress_classdiagram_spec_19_should_render_a_simple_class_diagram_with_notes_017.mmd`
    - `fixtures/class/upstream_cypress_classdiagram_v2_spec_18b_should_render_a_simple_class_diagram_with_notes_023.mmd`
    - `fixtures/class/upstream_cypress_classdiagram_v3_spec_18b_should_render_a_simple_class_diagram_with_notes_031.mmd`
    - `fixtures/class/upstream_cypress_classdiagram_elk_v3_spec_elk_18b_should_render_a_simple_class_diagram_with_notes_031.mmd`
    - `fixtures/class/upstream_separators_labels_notes.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter notes_ --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --dom-mode strict --dom-decimals 3`

- [x] Class note rank-order / placement parity (`classRenderer-v2.ts` / Dagre ordering)  
  Gap check:
  - Canonicalize the note-heavy TB tie case where our Dagre-ish layout picks the horizontally mirrored solution instead of Mermaid's left-leaning arrangement.
  - Keep the fix narrow: only mirror TB/no-namespace layouts with multiple attached notes that all resolve to the same-or-right side before the note-heavy canonicalization pass.
  - Match Mermaid DOM emission order for note-heavy fixtures by rendering note edges/empty note labels before relation edges/labels, and fill in the last note/relation width overrides needed by these fixtures.
  - Status: `stress_class_notes_and_keywords_003` and `stress_class_notes_wrap_positions_014` are now strict-green. The `notes_` strict XML filter is fully green (`0` mismatches), and full class strict mismatches drop from `196` to `194`.
  Evidence:
  - Fixtures:
    - `fixtures/class/stress_class_notes_and_keywords_003.mmd`
    - `fixtures/class/stress_class_notes_wrap_positions_014.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-svg-xml --check --diagram class --filter stress_class_notes_and_keywords_003 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --check --diagram class --filter stress_class_notes_wrap_positions_014 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --check --diagram class --filter notes_ --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-svg-xml --check --diagram class --dom-mode strict --dom-decimals 3`

- [x] `classDef default` + node-id `style default` with `htmlLabels: true`  
  Gap check:
  - Confirm nodes without explicit classes still receive implicit `classDef default` styling.
  - Confirm a node literally named `default` still layers node-id `style default ...` overrides on top.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_html_labels_default_class_077.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter stress_flowchart_html_labels_default_class_077`
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_html_labels_default_class_077`
    - `cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter stress_flowchart_html_labels_default_class_077 --dom-mode strict --dom-decimals 3`

- [x] `foreignObject` font-size inheritance (Mermaid CLI / Puppeteer)  
  Gap check:
  - Add a fixture that overrides `fontSize` and `themeVariables.fontSize` while forcing HTML labels.
  - Confirm upstream Mermaid CLI still measures at the browser default (16px) for the HTML label content.
  Evidence:
  - Class: `fixtures/class/stress_class_font_size_precedence_024.mmd`
    - Upstream: `fixtures/upstream-svgs/class/stress_class_font_size_precedence_024.svg`
  - Mindmap: `fixtures/mindmap/stress_mindmap_font_size_precedence_037.mmd`
    - Upstream: `fixtures/upstream-svgs/mindmap/stress_mindmap_font_size_precedence_037.svg`
  Notes:
  - This matches what we observe in Mermaid CLI baselines: HTML label contents do not reliably inherit SVG-root
    `font-size` CSS rules, so the effective label size is often the browser default (16px).

- [x] Font-size precedence rules per diagram (SVG text vs HTML labels)  
  Gap check:
  - Search docs: `docs/alignment/*MINIMUM.md` and per-diagram render modules for `fontSize`.
  - Confirm whether each diagram reads from `themeVariables.fontSize`, top-level `fontSize`, or a diagram override.
  Evidence:
  - A “fontSize smoke fixture” per diagram with `init` directives.
  - Timeline (theme `base`, `themeVariables.fontSize: "24px"` vs `fontSize: 10`):
    - Fixture: `fixtures/timeline/timeline_stress_font_size_precedence.mmd`
    - Compare: `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-decimals 3 --filter timeline_stress_font_size_precedence`
  Evidence (partial):
  - Class (SVG labels): Mermaid’s `createText(..., { width: calculateTextWidth(text, config) + 50 })` uses the
    top-level `fontSize` for the width probe, while the rendered SVG `<text>` inherits the root `font-size`
    (typically from `themeVariables.fontSize`). If those differ, upstream can wrap/split unexpectedly.
    - Fixture: `fixtures/class/stress_class_svg_font_size_precedence_025.mmd`
    - Upstream: `fixtures/upstream-svgs/class/stress_class_svg_font_size_precedence_025.svg`
  - Class (SVG labels, `themeVariables.fontSize` px-string parsing): upstream accepts `themeVariables.fontSize`
    in `"NNpx"` form; treating this as “missing” falls back to defaults and subtly shifts wrap boundaries.
    - Fixture: `fixtures/class/stress_class_svg_font_size_px_string_precedence_026.mmd`
    - Upstream: `fixtures/upstream-svgs/class/stress_class_svg_font_size_px_string_precedence_026.svg`
    - Compare: `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter stress_class_svg_font_size_px_string_precedence_026`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/class_root_overrides_11_12_2.rs`.
  - Flowchart: `fixtures/flowchart/stress_flowchart_font_size_precedence_073.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_font_size_precedence_073`
    - Note: root `viewBox` is pinned via `crates/merman-render/src/generated/flowchart_root_overrides_11_12_2.rs` for this fixture.
  - ER: `fixtures/er/stress_er_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3 --filter stress_er_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_er_font_size_precedence_001`
  - State: `fixtures/state/stress_state_font_size_precedence_071.mmd`
    - Compare: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3 --filter stress_state_font_size_precedence_071`
    - Compare (root): `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_state_font_size_precedence_071`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/state_root_overrides_11_12_2.rs` for this fixture.
  - Sequence: `fixtures/sequence/stress_sequence_font_size_precedence_090.mmd`
    - Compare: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-decimals 3 --filter stress_sequence_font_size_precedence_090`
    - Compare (root): `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_sequence_font_size_precedence_090`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/sequence_root_overrides_11_12_2.rs` for this fixture.
  - Sequence: `fixtures/sequence/upstream_cypress_sequencediagram_spec_should_render_with_an_init_directive_049.mmd`
    - Compare: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter should_render_with_an_init_directive_049`
  - Gantt: `fixtures/gantt/stress_gantt_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-decimals 3 --filter stress_gantt_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_gantt_font_size_precedence_001`
  - Journey: `fixtures/journey/stress_journey_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-decimals 3 --filter stress_journey_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_journey_font_size_precedence_001`
  - Block: `fixtures/block/stress_block_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-block-svgs --check-dom --dom-decimals 3 --filter stress_block_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_block_font_size_precedence_001`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/block_root_overrides_11_12_2.rs`.
  - Radar: `fixtures/radar/stress_radar_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-decimals 3 --filter stress_radar_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_radar_font_size_precedence_001`
    - Note: current smoke passes without extra layout/code changes.
  - Requirement: `fixtures/requirement/stress_requirement_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-decimals 3 --filter stress_requirement_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_requirement_font_size_precedence_001`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/requirement_root_overrides_11_12_2.rs`.
  - Kanban: `fixtures/kanban/stress_kanban_font_size_precedence_098.mmd`
    - Compare: `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-decimals 3 --filter stress_kanban_font_size_precedence_098`
    - Compare (root): `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_kanban_font_size_precedence_098`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/kanban_root_overrides_11_12_2.rs`.
  - GitGraph: `fixtures/gitgraph/stress_gitgraph_font_size_precedence_098.mmd`
    - Compare: `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-decimals 3 --filter stress_gitgraph_font_size_precedence_098`
    - Compare (root): `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_gitgraph_font_size_precedence_098`
    - Note: root `viewBox`/`max-width` is pinned via `crates/merman-render/src/generated/gitgraph_root_overrides_11_12_2.rs`.
  - Treemap: `fixtures/treemap/stress_treemap_font_size_precedence_001.mmd`
    - Compare: `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-decimals 3 --filter stress_treemap_font_size_precedence_001`
    - Compare (root): `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_treemap_font_size_precedence_001`
    - Note: current smoke passes without extra layout/code changes.
  Status:
  - Full corpus now passes both `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
    and `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 6`.

## C) Markdown subset parity

- [x] `_` delimiter correctness (`a__b`, `_a_b_`, `_a__b_`)  
  Gap check:
  - Confirm existing tests cover underscore-heavy ids and labels:
    - `docs/alignment/FLOWCHART_*`, `crates/merman-render/src/text/tests.rs`
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_markdown_underscore_delims_074.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter stress_flowchart_markdown_underscore_delims_074`
    - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_markdown_underscore_delims_074`
  - Unit test: `crates/merman-render/src/text/tests.rs` (`markdown_underscore_delimiters_match_mermaid`)

- [x] Inline code suppresses emphasis parsing  
  Gap check:
  - Ensure `` `**not bold**` `` remains literal in both SVG-label and HTML-label modes.
  Evidence (partial):
  - Class HTML labels preserve backticks and do not parse `**...**` inside them:
    - Fixture: `fixtures/class/stress_class_markdown_inline_code_022.mmd`
    - Compare: `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter stress_class_markdown_inline_code_022`
    - Compare (root): `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter stress_class_markdown_inline_code_022`
  - ER HTML labels preserve backticks and do not synthesize `<code>` / `<strong>` inside them:
    - Fixture: `fixtures/er/stress_er_entity_label_inline_code_004.mmd`
    - Compare: `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3 --filter stress_er_entity_label_inline_code_004`
    - Compare (root): `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_er_entity_label_inline_code_004`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram er --filter stress_er_entity_label_inline_code_004 --dom-mode strict --dom-decimals 3`
  - Tokenizer behavior:
    - Unit test: `crates/merman-render/src/text/tests.rs` (`markdown_inline_code_suppresses_emphasis_delimiters`)
  - Flowchart pipe edge labels keep bare backticks literal (instead of entering Markdown-string mode), across both `htmlLabels` paths:
    - Fixtures: `fixtures/flowchart/probe_flowchart_edge_markdown_html_true_981.mmd`, `fixtures/flowchart/probe_flowchart_edge_markdown_html_false_982.mmd`, `fixtures/flowchart/probe_flowchart_edge_markdown_partial_star_983.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter probe_flowchart_edge_markdown_`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter probe_flowchart_edge_markdown_ --dom-mode strict --dom-decimals 3`
  - Flowchart quoted Markdown edge labels normalize closing `</br>` like Mermaid and keep inline raw HTML split correctly across `htmlLabels` modes:
    - Fixtures: `fixtures/flowchart/probe_flowchart_edge_quoted_markdown_html_true_984.mmd`, `fixtures/flowchart/probe_flowchart_edge_quoted_markdown_html_false_985.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter probe_flowchart_edge_quoted_markdown_`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter probe_flowchart_edge_quoted_markdown_ --dom-mode strict --dom-decimals 3`
    - Unit tests: `crates/merman-render/src/text/tests.rs` (`flowchart_label_metrics_for_layout_measures_markdown_inline_html_like_mermaid`, `markdown_svg_wrapping_keeps_raw_html_tags_literal_but_wraps_like_mermaid`)

- [x] Partial `**...*` star runs in HTML labels follow Mermaid/CommonMark semantics  
  Gap check:
  - Ensure malformed strong-open + single-star-close sequences render as literal `*` plus `<em>...</em>` instead of staying fully literal.
  - Confirm the class member/classifier interaction (`+inline: **bold**` -> display text `+inline: **bold*`) matches upstream DOM.
  Evidence:
  - Fixture: `fixtures/class/stress_class_markdown_member_strong_023.mmd`
  - Compare:
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_class_markdown_member_strong_023`
    - `cargo run -p xtask -- compare-svg-xml --diagram class --filter stress_class_markdown_member_strong_023 --dom-mode strict --dom-decimals 3`
    - `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6`
  - Unit tests:
    - `crates/merman-render/src/text/tests.rs` (`markdown_html_label_fragment_reinterprets_partial_star_strong_like_mermaid`)
    - `crates/merman-render/src/text/tests.rs` (`markdown_xhtml_label_fragment_reinterprets_partial_star_strong_like_mermaid`)

- [x] Mixed paragraph + raw-block Markdown keeps Mermaid HTML-label semantics  
  Gap check:
  - Ensure `htmlLabels: true` Markdown labels preserve a leading paragraph as `<p>...</p>` while
    following raw/list-style lines stay literal text (browser-collapsed), instead of becoming
    extra `<br/>` lines.
  Evidence:
  - Flowchart fixture: `fixtures/flowchart/stress_flowchart_markdown_mixed_raw_blocks_078.mmd`
    - Compare:
      - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter stress_flowchart_markdown_mixed_raw_blocks_078`
      - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_flowchart_markdown_mixed_raw_blocks_078`
      - `cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter stress_flowchart_markdown_mixed_raw_blocks_078 --dom-mode strict --dom-decimals 3`
  - Mindmap fixture: `fixtures/mindmap/stress_mindmap_markdown_mixed_raw_blocks_038.mmd`
    - Compare:
      - `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-decimals 3 --filter stress_mindmap_markdown_mixed_raw_blocks_038`
      - `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_mindmap_markdown_mixed_raw_blocks_038`
      - `cargo run -p xtask -- compare-svg-xml --diagram mindmap --filter stress_mindmap_markdown_mixed_raw_blocks_038 --dom-mode strict --dom-decimals 3`
  - Unit test: `crates/merman-render/src/text/tests.rs` (`markdown_html_label_fragment_collapses_mixed_list_blocks_like_browser_dom`)

- [x] Inline `<br/>` + list-like continuation stays inside the same HTML-label paragraph
  Gap check:
  - Ensure Mermaid keeps `Hello<br/>- l1<br/>- l2` inside a single `<p>...</p>` for htmlLabels,
    instead of treating the `- ...` lines as raw/list blocks or escaping `<br/>` literally.
  Evidence:
  - Class fixture: `fixtures/class/stress_class_edge_label_markdown_br_listish_027.mmd`
    - Compare: `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter stress_class_edge_label_markdown_br_listish_027`
    - Compare (root): `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_class_edge_label_markdown_br_listish_027`
  - State fixture: `fixtures/state/stress_state_edge_label_markdown_br_listish_072.mmd`
    - Compare: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3 --filter stress_state_edge_label_markdown_br_listish_072`
  - Requirement fixture: `fixtures/requirement/stress_requirement_markdown_br_listish_150.mmd`
    - Compare: `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-decimals 3 --filter stress_requirement_markdown_br_listish_150`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram requirement --filter stress_requirement_markdown_br_listish_150 --dom-mode strict --dom-decimals 3`
  - Unit test: `crates/merman-render/src/text/tests.rs` (`markdown_xhtml_label_fragment_preserves_inline_br_listish_continuations`)

- [x] ER relationship HTML labels preserve Mermaid Markdown emphasis semantics
  Gap check:
  - Ensure `htmlLabels: true` ER relationship labels route through Mermaid `markdownToHTML()` semantics,
    so `**...**` / `_..._` become `<strong>` / `<em>` in `<foreignObject>` output and use the same
    markdown-aware bbox measurement for layout spacing.
  Evidence:
  - Fixture: `fixtures/er/stress_er_edge_label_markdown_002.mmd`
    - Compare: `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3 --filter stress_er_edge_label_markdown_002`
    - Compare (root): `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_er_edge_label_markdown_002`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram er --filter stress_er_edge_label_markdown_002 --dom-mode strict --dom-decimals 3`

- [x] Escaped entities survive markdown→HTML→SVG pipeline  
  Gap check:
  - Add a fixture containing `&lt;`, `&amp;`, and unknown `&entity;` sequences.
  Evidence:
  - Rendered SVG must remain valid XML and match upstream escaping behavior.
  - Fixture: `fixtures/timeline/timeline_stress_unknown_xml_entity.mmd`
    - Compare: `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-decimals 3 --filter timeline_stress_unknown_xml_entity`

## D) Theme & CSS selector drift

- [x] Verify `theme=default` vs `base` does not cause implicit defaults  
  Gap check:
  - Add paired xychart + flowchart fixtures using `theme: default` and `theme: base` (no explicit themeVariables) so
    default palette/background drift is observable.
  Evidence:
  - Flowchart fixtures:
    - `fixtures/flowchart/stress_flowchart_theme_default_vs_base_default_074.mmd`
    - `fixtures/flowchart/stress_flowchart_theme_default_vs_base_base_075.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter theme_default_vs_base`
  - XYChart fixtures:
    - `fixtures/xychart/stress_xychart_theme_default_vs_base_default_001.mmd`
    - `fixtures/xychart/stress_xychart_theme_default_vs_base_base_002.mmd`
    - Compare: `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-decimals 3 --filter theme_default_vs_base`

- [x] Inline `classDef` / `style` overrides: font-family/font-size/opacity  
  Gap check:
  - Add fixtures that apply `font-family`, `font-size`, and `opacity` via both `classDef` and `style`.
  Evidence:
  - Flowchart fixture:
    - `fixtures/flowchart/stress_flowchart_text_style_overrides_076.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter text_style_overrides`
  - State fixture:
    - `fixtures/state/stress_state_text_style_overrides_070.mmd`
    - Compare: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3 --filter text_style_overrides`

## E) SVG DOM stability and IDs

- [x] Stable element ordering (nodes/edges/clusters)  
  Gap check:
  - If strict diffs are noisy, verify dom-mode parity is stable and still catches ordering drift.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`

- [x] Marker/id/url references are stable and correctly escaped  
  Gap check:
  - Ensure identifier-like attrs (e.g. `id`, `href`, `xlink:href`, `aria-*`) are normalized for Mermaid-generated ids
    so DOM parity remains robust to upstream randomness while still validating escaping.
  Evidence:
  - Normalization: `crates/xtask/src/svgdom.rs` (identifier token normalization for compare dom-mode parity).
  - Compare: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`

## F) Subgraphs, clipping, edge geometry

- [x] Boundary clipping for cluster edges  
  Gap check:
  - Use an existing stress fixture for cluster-as-endpoint edges; confirm no DOM/viewport drift in parity-root mode.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_edges_to_from_subgraphs_017.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter stress_flowchart_edges_to_from_subgraphs_017`

- [x] Edge labels near cluster titles  
  Gap check:
  - Validate label placement does not overlap titles in parity-root mode.
  Evidence:
  - Fixture: `fixtures/flowchart/stress_flowchart_edge_label_near_cluster_title_018.mmd`
    - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter stress_flowchart_edge_label_near_cluster_title_018`

## G) Diagram-specific hardening passes

- [x] Sequence: note wrapping / activation stacking / message font precedence  
  Gap check:
  - Ensure wrapped notes/messages, stacked activations, and message font precedence are parity-gated.
  Evidence:
  - Fixture: `fixtures/sequence/activation_stacked.mmd`
    - Compare: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter activation_stacked`

- [x] Gantt: date parsing/timezone + “today” determinism  
  Gap check:
  - Confirm “today” marker rendering is compared using a frozen `now` derived from the upstream SVG baseline.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter today_marker`

- [x] Class: generics + namespace/member layout  
  Gap check:
  - Ensure generics and namespaces are covered via upstream fixtures and checked in parity-root mode.
  Evidence:
  - Fixture: `fixtures/class/upstream_docs_classdiagram_generic_types_018.mmd`
    - Compare: `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter generic_types_018`
  - Fixture: `fixtures/class/stress_class_nested_namespaces_cross_edges_008.mmd`
    - Compare: `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter nested_namespaces_cross_edges_008`

- [x] Class: remaining strict-XML generic bucket  
  Gap check:
  - Preserve relation-only / namespace-attached generic type params in the typed class model and pin the last HTML-label generic repeat offenders against Mermaid strict XML.
  - Re-run the generic strict bucket after each fix and keep the full-class strict count trending down.
  Evidence:
  - Strict XML generic filter is now green (`cargo run -p xtask -- compare-svg-xml --check --diagram class --filter generic --dom-mode strict --dom-decimals 3`).
  - Green fixtures:
    - `upstream_cypress_classdiagram_v3_spec_7_should_render_a_simple_class_diagram_with_generic_class_014`
    - `upstream_cypress_classdiagram_v3_spec_8_should_render_a_simple_class_diagram_with_generic_class_and_re_016`
    - `upstream_cypress_classdiagram_v3_spec_12_should_render_a_simple_class_diagram_with_generic_types_021`
    - `stress_class_nested_generics_static_013`
    - `stress_class_member_types_arrays_generics_022`
    - `upstream_docs_classdiagram_generic_types_018`
    - `upstream_namespaces_and_generics`
    - `stress_class_interfaces_generics_dependencies_018`
    - `stress_class_dense_namespaces_generics_001`
  - Additional green fixtures: `stress_class_nested_namespaces_many_levels_021`, `stress_class_comments_inside_namespaces_024`.
  - Mermaid recursive namespace parity now injects extracted cluster roots before placeholder sizing and keeps multi-root nested namespace wrappers localized like upstream.
  - Full class strict count is now `128` mismatches.
  - Relation/cardinality bucket status: terminal `foreignObject` sizing now matches Mermaid's `value.length * 9` rule, relation titles decode entities exactly once (`< owns` no longer double-escapes), terminal layout now keeps Mermaid's 10px marker gap even on plain association ends, `edgeLabels` DOM now matches upstream's `edgeLabel*`-then-`edgeTerminals*` ordering, and class edge-label placement now follows Mermaid's `positionEdgeLabel(updatedPath ? calcLabelPosition(path) : edge.x/y)` midpoint logic when the rendered `curveBasis` path no longer contains the raw mid control point.
  - Additional title/member/relation-label width overrides (`Class02..24`, `Order`, `Payment`, `Person`, `references`, `reads`, `feedback`, etc.) have collapsed the old layout-width drift into mostly 0.001-level path output differences.
  - Raw-SVG precision overrides now cover the last repeat offenders in this bucket (`Order`, `Payment`, `Driver`, `Wheel`, `owns`, `references`, `emits`, `feedback`, `+bar : int`, `+foo : bool`), mixed end-only terminal labels keep Mermaid's DOM order, and the single-character `E` title reuses its known 60px HTML cap instead of the generic 61px fallback.
  - This relation/cardinality repeat-offender set is now strict-green for `stress_class_parallel_edges_and_cardinality_004`, `stress_class_association_aggregation_composition_019`, `stress_class_many_relations_labels_020`, `upstream_cross_namespace_relations_spec`, and `upstream_relation_types_and_cardinalities_spec`; the remaining `class` strict mismatches have shifted back to other buckets (markdown / htmlLabels / interface-enum / unicode fixtures).
  - Annotation-driven HTML node bounds now reuse the same known rendered-width overrides during layout that render-time `foreignObject` emission already uses; this makes `upstream_annotations_in_brackets_spec`, `stress_class_interfaces_and_abstracts_007`, and `stress_class_member_separators_and_annotations_009` strict-green and removes the last 1/128px class box drift in that bucket.
  - The next enum/interface mix repeat offender is also green: `stress_class_enums_and_interfaces_mix_023` now uses Mermaid-matching HTML caps for `Status`, `UNKNOWN`, and `+run() : Status`, which also collapses the remaining `Status` node width drift.

- [x] Class: strict-XML unicode + namespace facade ordering  
  Gap check:
  - Ensure unicode/CJK/RTL/emoji labels do not drift node sizing or edge routes under strict canonical XML.
  - Ensure note nodes and namespace-qualified facade nodes have stable same-rank ordering in Dagre tie cases.
  Evidence:
  - Strict XML unicode filter is now green (`cargo run -p xtask -- compare-svg-xml --check --diagram class --filter unicode --dom-mode strict --dom-decimals 3`).
  - Green fixtures:
    - `fixtures/class/stress_class_unicode_and_entities_012.mmd`
    - `fixtures/class/stress_class_unicode_namespace_mix_017.mmd`

- [ ] Markdown + `htmlLabels` repeat offenders sweep (flowchart/class first)  
  Gap check:
  - Focus on label-tokenization edge cases (`__`, nested emphasis, backticks/code spans, list-like lines, trailing spaces).
  - For HTML labels: confirm `foreignObject` width vs `max-width`, font-size inheritance quirks, and wrap boundary parity.
  Suggested triage loop:
  - Bucket by fixture name: `--filter markdown`, `--filter html_labels`, `--filter htmlLabels`
  - Validate with both structural and geometry-sensitive modes:
    - `cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-decimals 3 --filter <bucket>`
    - `cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter <bucket>`
    - `cargo run -p xtask -- compare-svg-xml --check --diagram <diagram> --filter <bucket> --dom-mode strict --dom-decimals 3`

- [x] State: composite padding + classDef html label measurement  
  Gap check:
  - Confirm classDef-driven HTML label metrics don’t drift in parity-root mode.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 6`

- [x] Mindmap: multiline CJK + deep nesting viewport drift  
  Gap check:
  - Ensure root viewport bounds include all labels; compare in parity-root mode.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 6`
  - Note: `stress_mindmap_font_size_precedence_037` is pinned via `crates/merman-render/src/generated/mindmap_root_overrides_11_12_2.rs`.

- [x] Architecture: `iconText` `foreignObject` HTML / link wrapper parity  
  Gap check:
  - Confirm root-level SVG-namespace `<a>` wrappers inside `iconText` split inline HTML descendants (`<code>`, `<span>`, `<b>`, etc.) the same way Mermaid CLI / Chromium serializes them from `foreignObject`.
  - Confirm singleton top-level `iconText` services keep Mermaid’s extra service Y offset / root `viewBox` shift in strict XML mode.
  Evidence:
  - Fixture: `fixtures/architecture/stress_architecture_icontext_anchor_code_044.mmd`
    - Compare: `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-decimals 3 --filter stress_architecture_icontext_anchor_code_044`
    - Compare (root): `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter stress_architecture_icontext_anchor_code_044`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram architecture --filter stress_architecture_icontext_anchor_code_044 --dom-mode strict --dom-decimals 3`
  - Fixtures: `fixtures/architecture/probe_architecture_icontext_anchor_*_99{1..8}.mmd`
    - Compare (strict XML): `cargo run -p xtask -- compare-svg-xml --diagram architecture --filter probe_architecture_icontext_ --dom-mode strict --dom-decimals 3`

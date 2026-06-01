use merman_core::{Engine, ParseOptions};
use merman_render::model::{LayoutDiagram, LayoutEdge, LayoutPoint, SequenceDiagramLayout};
use merman_render::svg::{
    SvgRenderOptions, render_sequence_diagram_debug_svg, render_sequence_diagram_svg,
};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;
#[cfg(feature = "ratex-math")]
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn extract_self_closing_tags<'a>(s: &'a str, tag_name: &str) -> Vec<&'a str> {
    let needle = format!("<{tag_name}");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = s[i..].find(&needle) {
        let start = i + pos;
        let Some(end_rel) = s[start..].find("/>") else {
            break;
        };
        let end = start + end_rel + 2;
        out.push(&s[start..end]);
        i = end;
    }
    out
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn render_sequence_svg_from_fixture(fixture: &str) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_sequence_svg_from_text(&text)
}

fn render_sequence_svg_from_fixture_with_options(
    fixture: &str,
    options: &SvgRenderOptions,
) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };

    render_sequence_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        options,
    )
    .expect("render svg")
}

fn render_sequence_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };

    render_sequence_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

#[test]
fn sequence_root_overrides_can_be_disabled_per_render_options() {
    let stem = "stress_wrap_directive_and_prefixes_028";
    let fixture = format!("{stem}.mmd");
    let enabled = render_sequence_svg_from_fixture_with_options(
        &fixture,
        &SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..SvgRenderOptions::default()
        },
    );
    let disabled = render_sequence_svg_from_fixture_with_options(
        &fixture,
        &SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            apply_root_overrides: false,
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        enabled.contains("max-width: 1022px;"),
        "expected retained root override to pin the Sequence root width"
    );
    assert!(
        !disabled.contains("max-width: 1022px;"),
        "expected disabled root overrides to emit computed Sequence root width"
    );
}

#[test]
fn sequence_autonumber_renders_decimal_sequence_numbers() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
autonumber 10.01 .01
Alice->>Bob:Hello
Bob-->>Alice:Back
Bob->>Alice:Again"#,
    );

    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.01</text>"#),
        "expected first decimal sequence number in SVG"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.02</text>"#),
        "expected second decimal sequence number rounded to hundredths"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.03</text>"#),
        "expected third decimal sequence number rounded to hundredths"
    );
    assert!(
        !svg.contains("10.019999"),
        "expected decimal sequence numbers to avoid floating point artifacts"
    );
}

#[test]
fn sequence_debug_svg_renders_generic_polyline_points() {
    let layout = SequenceDiagramLayout {
        nodes: Vec::new(),
        clusters: Vec::new(),
        bounds: None,
        edges: vec![LayoutEdge {
            id: "generic-edge".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: -0.0, y: 0.0 },
                LayoutPoint { x: 1.5, y: -2.0 },
                LayoutPoint { x: 3.25, y: 4.5 },
            ],
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        }],
    };

    let svg = render_sequence_diagram_debug_svg(&layout, &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"<polyline class="edge" points="0,0 1.5,-2 3.25,4.5" />"#),
        "expected generic sequence debug edges to render a shared-helper point list"
    );
}

#[test]
fn sequence_note_width_expands_for_literal_br_backslash_t_in_vendored_mode() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("html_br_variants_and_wrap.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(
            merman_render::text::VendoredFontMetricsTextMeasurer::default(),
        ),
        ..LayoutOptions::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };

    let note = layout
        .nodes
        .iter()
        .find(|n| n.id == "note-7")
        .expect("expected note-7 layout node");

    // Mermaid's text-dimension probe treats the escaped `<br \t/>` as literal single-run text,
    // then adds the normal note padding.
    assert_eq!(note.width, 151.0);
}

#[test]
fn sequence_alt_multiple_elses_separators_touch_frame_edges() {
    let svg = render_sequence_svg_from_fixture("upstream_alt_multiple_elses_spec.mmd");

    let line_tags = extract_self_closing_tags(&svg, "line");
    let loop_lines: Vec<&str> = line_tags
        .into_iter()
        .filter(|t| t.contains(r#"class="loopLine""#))
        .collect();

    let dashed_separators: Vec<&str> = loop_lines
        .iter()
        .copied()
        .filter(|t| t.contains("stroke-dasharray: 3, 3"))
        .collect();
    assert_eq!(
        dashed_separators.len(),
        2,
        "expected 2 dashed separators for 3 alt sections"
    );

    let y0 = attr_f64(dashed_separators[0], "y1").expect("sep y1");
    let y1 = attr_f64(dashed_separators[1], "y1").expect("sep y1");
    assert!(
        (y0 - y1).abs() > 0.0001,
        "expected separators to have distinct y"
    );

    let mut frame_min_x = f64::INFINITY;
    let mut frame_max_x = f64::NEG_INFINITY;
    for t in &loop_lines {
        if t.contains("style=") {
            continue;
        }
        let (Some(x1), Some(x2)) = (attr_f64(t, "x1"), attr_f64(t, "x2")) else {
            continue;
        };
        if (x1 - x2).abs() <= 0.0001 {
            frame_min_x = frame_min_x.min(x1);
            frame_max_x = frame_max_x.max(x1);
        }
    }
    assert!(frame_min_x.is_finite() && frame_max_x.is_finite());

    for sep in dashed_separators {
        let x1 = attr_f64(sep, "x1").expect("sep x1");
        let x2 = attr_f64(sep, "x2").expect("sep x2");
        assert!(
            x1 <= frame_min_x + 0.0001,
            "expected separator x1 ({x1}) to touch frame left edge ({frame_min_x})"
        );
        assert!(
            x2 >= frame_max_x - 0.0001,
            "expected separator x2 ({x2}) to touch frame right edge ({frame_max_x})"
        );
    }
}

#[test]
fn sequence_rect_block_is_root_level_before_actors() {
    let svg = render_sequence_svg_from_fixture("upstream_rect_block_spec.mmd");

    let fill_pos = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected rect fill to match directive payload");
    let rect_pos = svg[..fill_pos]
        .rfind("<rect")
        .expect("expected rect tag for fill");
    let rect_end_rel = svg[rect_pos..]
        .find("/>")
        .expect("expected self-closing rect tag");
    let rect_tag = &svg[rect_pos..(rect_pos + rect_end_rel + 2)];
    assert!(rect_tag.contains(r#"class="rect""#), "expected rect class");

    let actor_pos = svg
        .find(r#"class="actor actor-bottom""#)
        .expect("expected bottom actors");
    assert!(
        rect_pos < actor_pos,
        "expected rect blocks to be emitted before actor groups"
    );
}

#[test]
fn sequence_nested_rect_blocks_render_in_start_order() {
    let svg = render_sequence_svg_from_fixture("upstream_nested_rect_blocks_spec.mmd");

    let outer = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected outer rect fill");
    let inner = svg
        .find(r#"fill="rgb(0, 0, 0)""#)
        .expect("expected inner rect fill");
    assert!(
        outer < inner,
        "expected nested rect blocks to be emitted in start order"
    );
}

#[test]
fn sequence_notes_render_inline_with_block_frames() {
    let svg = render_sequence_svg_from_fixture("stress_end_in_labels_025.mmd");

    let loop_pos = svg
        .find("[health(end)check]")
        .expect("expected loop frame label");
    let note_pos = svg.find(r#"class="note""#).expect("expected note group");
    let alt_pos = svg
        .find("[should continue]")
        .expect("expected alt frame label");

    assert!(
        loop_pos < note_pos,
        "expected completed loop frame to render before the later note"
    );
    assert!(
        note_pos < alt_pos,
        "expected note to render before its enclosing alt frame closes"
    );
}

#[test]
fn sequence_notes_expand_viewbox_left_for_leftof_notes() {
    let svg = render_sequence_svg_from_fixture("notes_placements.mmd");
    assert!(
        svg.contains(r#"viewBox="-150 -10"#),
        "expected viewBox min_x to expand for left-of notes"
    );
    assert!(
        svg.contains(r#"max-width: 750px"#),
        "expected max-width to reflect expanded viewBox width"
    );
}

#[test]
fn sequence_frontmatter_title_expands_layout_root_y() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_html_demos_sequence_sequence_diagram_demos_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.title.as_deref(), Some("With forced menus"));
    assert!(
        parsed
            .model
            .get("title")
            .is_none_or(|title| title.is_null()),
        "frontmatter title should stay in parse metadata, not the sequence semantic title"
    );

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };
    let bounds = layout.bounds.as_ref().expect("sequence root bounds");
    assert_eq!(bounds.min_y, -50.0);
}

#[test]
fn sequence_message_font_size_override_matches_mermaid_cli_baselines() {
    // Mermaid CLI (mmdc) currently does not reflect `sequence.messageFontSize` overrides in the
    // emitted SVG; it sticks to the global `fontSize` defaults. Keep our Stage B output aligned
    // with the upstream baselines under `fixtures/upstream-svgs/sequence`.
    let svg = render_sequence_svg_from_fixture(
        "upstream_cypress_sequencediagram_spec_should_render_different_message_fonts_when_configured_011.mmd",
    );
    assert!(
        svg.contains("font-size: 16px"),
        "expected message/actor text to use the global fontSize (16px) like Mermaid CLI baselines"
    );
    assert!(
        !svg.contains("font-size: 18px"),
        "expected sequence.messageFontSize (18px) to not affect SVG output under the pinned upstream baselines"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn sequence_svg_renders_ratex_math_message_and_note_end_to_end() {
    let text = r#"sequenceDiagram
participant A
participant B
A->>B: $$x^2$$
Note right of B: $$x^2$$
"#;
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let layout_options = LayoutOptions::default().with_math_renderer(math_renderer.clone());
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };

    let svg = render_sequence_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            math_renderer: Some(math_renderer),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG sizing in sequence labels: {svg}"
    );
    assert!(
        svg.contains(r#"<div style="width: fit-content;""#),
        "expected Sequence math labels to use the KaTeX foreignObject shell: {svg}"
    );
    assert!(
        svg.contains("<path"),
        "expected RaTeX glyph paths in sequence SVG: {svg}"
    );
    assert!(
        !svg.contains("$$x^2$$"),
        "expected math source delimiters to be replaced by rendered SVG: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn sequence_docs_math_fixture_renders_supported_ratex_formulas() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_docs_math_sequence_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let layout_options = LayoutOptions::default().with_math_renderer(math_renderer.clone());
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::SequenceDiagram(layout) = &out.layout else {
        panic!("expected SequenceDiagram layout");
    };

    let svg = render_sequence_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            math_renderer: Some(math_renderer),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    let inline_formula_count = svg
        .matches(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 "#)
        .count();
    assert!(
        inline_formula_count >= 7,
        "expected participant, message, and note math labels to render through RaTeX: {svg}"
    );
    assert!(
        !svg.contains(r#"Solve: $$\sqrt{2+2}$$"#) && !svg.contains(r#"Answer: $$2$$"#),
        "expected mixed sequence message formulas to replace source delimiters: {svg}"
    );
}

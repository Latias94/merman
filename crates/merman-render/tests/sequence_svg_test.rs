use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_sequence_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

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

    // Mermaid@11.12.2 widens the note box to fit the literal `<br \\t/>` markup, yielding a 152px
    // note width in strict SVG XML baselines.
    assert_eq!(note.width, 152.0);
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

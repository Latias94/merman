use merman_core::{Engine, ParseOptions};
use merman_render::model::{LayoutDiagram, PieDiagramLayout};
use merman_render::svg::{SvgRenderOptions, render_pie_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn layout_pie_from_text(text: &str) -> PieDiagramLayout {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::PieDiagram(layout) = out.layout else {
        panic!("expected PieDiagram layout");
    };
    *layout
}

fn render_pie_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::PieDiagram(layout) = &out.layout else {
        panic!("expected PieDiagram layout");
    };

    render_pie_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok")
}

#[test]
fn pie_slices_follow_input_order_in_mermaid_11_15() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 50
"#,
    );

    let labels: Vec<&str> = layout
        .slices
        .iter()
        .map(|slice| slice.label.as_str())
        .collect();

    assert_eq!(labels, vec!["A", "B", "C"]);
}

#[test]
fn pie_hidden_slices_still_reserve_color_domain_slots() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 0.1
  "D" : 50
"#,
    );

    let slices: Vec<(&str, &str)> = layout
        .slices
        .iter()
        .map(|slice| (slice.label.as_str(), slice.fill.as_str()))
        .collect();

    assert_eq!(
        slices,
        vec![
            ("A", "#ECECFF"),
            ("B", "#ffffde"),
            ("D", "hsl(240, 100%, 86.2745098039%)")
        ]
    );
}

#[test]
fn pie_text_position_config_moves_slice_labels() {
    let layout = layout_pie_from_text(
        r#"%%{init: {"pie": {"textPosition": 0.5}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let first = layout
        .slices
        .iter()
        .find(|slice| slice.label == "A")
        .expect("slice A exists");

    assert!((first.text_x - 92.5).abs() < 1e-9);
    assert!(first.text_y.abs() < 1e-9);
}

#[test]
fn pie_donut_hole_config_renders_annular_slice_paths() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 0.4}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        svg.contains("A74,74"),
        "expected inner-radius arc in donut slice path: {svg}"
    );
    assert!(
        !svg.contains("L0,0Z"),
        "donut slices should not close through the center: {svg}"
    );
}

#[test]
fn pie_invalid_donut_hole_config_falls_back_to_solid_slices() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 1.2}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        !svg.contains("A222,222"),
        "invalid donutHole should not be used as an inner radius: {svg}"
    );
    assert!(
        svg.contains("L0,0Z"),
        "invalid donutHole should fall back to solid slices: {svg}"
    );
}

#[test]
fn pie_legend_position_config_controls_layout_regions() {
    let diagram = |position: &str| {
        layout_pie_from_text(&format!(
            r#"%%{{init: {{"pie": {{"legendPosition": "{position}"}}}}}}%%
pie
  "A" : 1
  "B" : 1
"#
        ))
    };

    let right = diagram("right");
    let right_bounds = right.bounds.as_ref().expect("right bounds");
    assert!(right_bounds.max_x > 490.0);
    assert_eq!(right_bounds.max_y, 450.0);
    assert_eq!(right.legend_x, 216.0);
    assert_eq!(right.legend_items[0].y, -22.0);

    let top = diagram("top");
    let top_bounds = top.bounds.as_ref().expect("top bounds");
    assert_eq!(top_bounds.max_x, 490.0);
    assert_eq!(top_bounds.max_y, 494.0);
    assert!(top.legend_x < 0.0);
    assert_eq!(top.legend_items[0].y, -185.0);

    let bottom = diagram("bottom");
    let bottom_bounds = bottom.bounds.as_ref().expect("bottom bounds");
    assert_eq!(bottom_bounds.max_x, 490.0);
    assert_eq!(bottom_bounds.max_y, 494.0);
    assert!(bottom.legend_x < 0.0);
    assert_eq!(bottom.legend_items[0].y, 207.0);

    let left = diagram("left");
    let left_bounds = left.bounds.as_ref().expect("left bounds");
    assert!(left_bounds.max_x > 490.0);
    assert_eq!(left_bounds.max_y, 450.0);
    assert_eq!(left.legend_x, -207.0);
    assert_eq!(left.legend_items[0].y, -22.0);

    let center = diagram("center");
    let center_bounds = center.bounds.as_ref().expect("center bounds");
    assert_eq!(center_bounds.max_x, 490.0);
    assert_eq!(center_bounds.max_y, 450.0);
    assert!(center.legend_x < 0.0);
    assert_eq!(center.legend_items[0].y, -22.0);
}

#[test]
fn pie_legend_position_top_and_left_move_the_pie_group() {
    let top_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "top"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    assert!(top_svg.contains(r#"viewBox="0 0 490 494""#));
    assert!(
        top_svg.contains(r#"<g transform="translate(0,66)">"#),
        "top legend should move the pie group below the legend: {top_svg}"
    );

    let left_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "left"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    assert!(
        left_svg.contains(r#"<g transform="translate(32.203125,0)">"#),
        "left legend should move the pie group right by legend width: {left_svg}"
    );
    assert!(left_svg.contains(r#"class="legend" transform="translate(-207,-22)""#));
}

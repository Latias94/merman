use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_svg_from_text(text: &str, diagram_id: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    render_layouted_svg(
        &out,
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg")
}

fn assert_scoped_marker(svg: &str, diagram_id: &str, local_id: &str) {
    let scoped_id = format!(r#"id="{diagram_id}-{local_id}""#);
    let scoped_url = format!(r#"url(#{diagram_id}-{local_id})"#);
    let bare_id = format!(r#"id="{local_id}""#);
    let bare_url = format!(r#"url(#{local_id})"#);

    assert!(
        svg.contains(&scoped_id),
        "expected scoped marker definition `{scoped_id}` in SVG:\n{svg}"
    );
    assert!(
        svg.contains(&scoped_url),
        "expected scoped marker reference `{scoped_url}` in SVG:\n{svg}"
    );
    assert!(
        !svg.contains(&bare_id),
        "expected no bare marker definition `{bare_id}` in SVG:\n{svg}"
    );
    assert!(
        !svg.contains(&bare_url),
        "expected no bare marker reference `{bare_url}` in SVG:\n{svg}"
    );
}

fn assert_scoped_definition_id(svg: &str, diagram_id: &str, local_id: &str) {
    let scoped_id = format!(r#"id="{diagram_id}-{local_id}""#);
    let bare_id = format!(r#"id="{local_id}""#);

    assert!(
        svg.contains(&scoped_id),
        "expected scoped definition `{scoped_id}` in SVG:\n{svg}"
    );
    assert!(
        !svg.contains(&bare_id),
        "expected no bare definition `{bare_id}` in SVG:\n{svg}"
    );
}

#[test]
fn c4_marker_ids_are_prefixed_with_diagram_svg_id() {
    let svg = render_svg_from_text(
        r#"C4Context
Person(customer, "Customer")
System(system, "System")
Rel(customer, system, "Uses")"#,
        "m15-c4",
    );

    assert_scoped_marker(&svg, "m15-c4", "arrowhead");
}

#[test]
fn journey_marker_ids_are_prefixed_with_diagram_svg_id() {
    let svg = render_svg_from_text(
        r#"journey
title My day
section Work
  Make tea: 5: Me
  Write code: 3: Me"#,
        "m15-journey",
    );

    assert_scoped_marker(&svg, "m15-journey", "arrowhead");
}

#[test]
fn timeline_marker_ids_are_prefixed_with_diagram_svg_id() {
    let svg = render_svg_from_text(
        r#"timeline
title Release
section Phase
  Alpha : Build
  Beta : Test"#,
        "m15-timeline",
    );

    assert_scoped_marker(&svg, "m15-timeline", "arrowhead");
}

#[test]
fn sequence_marker_ids_are_prefixed_with_diagram_svg_id_and_css_uses_suffix_selectors() {
    let svg = render_svg_from_text(
        r#"sequenceDiagram
autonumber
Alice->>Bob: Hello
Bob-->>Alice: Back"#,
        "m15-sequence",
    );

    assert_scoped_marker(&svg, "m15-sequence", "arrowhead");
    assert_scoped_marker(&svg, "m15-sequence", "sequencenumber");
    assert_scoped_definition_id(&svg, "m15-sequence", "computer");
    assert_scoped_definition_id(&svg, "m15-sequence", "database");
    assert_scoped_definition_id(&svg, "m15-sequence", "clock");
    assert_scoped_definition_id(&svg, "m15-sequence", "solidTopArrowHead");
    assert_scoped_definition_id(&svg, "m15-sequence", "solidBottomArrowHead");
    assert_scoped_definition_id(&svg, "m15-sequence", "stickTopArrowHead");
    assert_scoped_definition_id(&svg, "m15-sequence", "stickBottomArrowHead");
    assert!(
        svg.contains(r#"data-et="life-line" data-id="Alice""#),
        "expected sequence lifeline data attributes:\n{svg}"
    );
    assert!(
        svg.contains(r#"data-et="message" data-id="i1" data-from="Alice" data-to="Bob""#),
        "expected sequence message data attributes:\n{svg}"
    );
    assert!(
        svg.contains(r#"[id$="-arrowhead"] path"#),
        "expected sequence CSS to target prefixed marker IDs by suffix:\n{svg}"
    );
    assert!(
        svg.contains(r#"[id$="-sequencenumber"]"#),
        "expected sequence CSS to target prefixed sequence number IDs by suffix:\n{svg}"
    );
    assert!(
        !svg.contains(r#"#arrowhead path"#),
        "expected no exact bare arrowhead CSS selector:\n{svg}"
    );
    assert!(
        !svg.contains(r#"#sequencenumber"#),
        "expected no exact bare sequence number CSS selector:\n{svg}"
    );
}

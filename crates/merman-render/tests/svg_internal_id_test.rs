use merman_core::{Engine, ParseOptions};
use merman_render::svg::{IconRegistry, IconSvg, SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};
use std::sync::Arc;

fn render_svg_from_text(text: &str, diagram_id: &str) -> String {
    render_svg_from_text_with_options(
        text,
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
    )
}

fn render_svg_from_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    render_layouted_svg(&out, layout_options.text_measurer.as_ref(), options).expect("render svg")
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

#[test]
fn gantt_task_and_exclude_ids_are_prefixed_with_diagram_svg_id() {
    let svg = render_svg_from_text(
        r#"gantt
dateFormat YYYY-MM-DD
excludes 2024-01-02
section Work
  Build: a1, 2024-01-01, 3d"#,
        "m15-gantt",
    );

    assert_scoped_definition_id(&svg, "m15-gantt", "a1");
    assert_scoped_definition_id(&svg, "m15-gantt", "a1-text");
    assert_scoped_definition_id(&svg, "m15-gantt", "exclude-2024-01-02");
}

#[test]
fn gantt_prototype_like_task_ids_are_prefixed_with_diagram_svg_id() {
    let svg = render_svg_from_text(
        r#"gantt
dateFormat YYYY-MM-DD
section Work
  Proto task: __proto__, 2024-01-01, 1d
  Ctor task: constructor, 2024-01-02, 1d"#,
        "m15-gantt-proto",
    );

    assert_scoped_definition_id(&svg, "m15-gantt-proto", "__proto__");
    assert_scoped_definition_id(&svg, "m15-gantt-proto", "__proto__-text");
    assert_scoped_definition_id(&svg, "m15-gantt-proto", "constructor");
    assert_scoped_definition_id(&svg, "m15-gantt-proto", "constructor-text");
}

#[test]
fn flowchart_iconify_internal_ids_are_scoped_per_node() {
    let mut registry = IconRegistry::new();
    registry.insert(
        "test:clip",
        IconSvg::new(
            r##"<defs><clipPath id="clip"><path id="shape" d="M0 0H16V16H0z"/></clipPath></defs><path data-icon="fixture" clip-path="url(#clip)" d="M0 0H16V16H0z"/><use href="#shape" xlink:href="#shape"/>"##,
            16.0,
            16.0,
        ),
    );

    let svg = render_svg_from_text_with_options(
        r#"flowchart TD
A@{ icon: "test:clip", label: "A" }
B@{ icon: "test:clip", label: "B" }
A --> B"#,
        &SvgRenderOptions {
            diagram_id: Some("m15-flowchart-icons".to_string()),
            icon_registry: Some(Arc::new(registry)),
            ..SvgRenderOptions::default()
        },
    );

    assert!(!svg.contains(r#"id="clip""#), "{svg}");
    assert!(!svg.contains(r#"id="shape""#), "{svg}");
    assert!(!svg.contains(r#"url(#clip)"#), "{svg}");
    assert!(!svg.contains(r##"href="#shape""##), "{svg}");
    assert_eq!(svg.matches(r#"data-icon="fixture""#).count(), 2, "{svg}");

    let ids = internal_iconify_ids(&svg);
    assert_eq!(ids.len(), 4, "{svg}");
    let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), ids.len(), "{svg}");
}

#[test]
fn architecture_builtin_icon_internal_ids_are_scoped_per_node() {
    let svg = render_svg_from_text(
        r#"architecture-beta
  service a(database)[A]
  service b(database)[B]
  a:R --> L:b"#,
        "m15-architecture-icons",
    );

    assert!(!svg.contains(r#"id="b""#), "{svg}");
    assert!(!svg.contains(r#"id="c""#), "{svg}");
    assert!(!svg.contains(r#"id="d""#), "{svg}");
    assert!(!svg.contains(r#"id="e""#), "{svg}");

    let ids = internal_iconify_ids(&svg);
    assert_eq!(ids.len(), 8, "{svg}");
    let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), ids.len(), "{svg}");
}

fn internal_iconify_ids(svg: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut index = 0;
    while let Some(relative) = svg[index..].find(r#"id="IconifyId"#) {
        let id_start = index + relative + r#"id=""#.len();
        let id_end = svg[id_start..].find('"').expect("id end") + id_start;
        ids.push(svg[id_start..id_end].to_string());
        index = id_end + 1;
    }
    ids
}

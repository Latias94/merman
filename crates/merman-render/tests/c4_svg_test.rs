use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_c4_svg_with_environment(source: &str, environment: &RenderEnvironment) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = environment.begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn layout_c4_with_options(source: &str, options: &LayoutOptions) -> C4DiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("C4 layout projection")
}

fn svg_text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn render_c4_with_preset(
    source: &str,
    preset: merman_render::presentation::HostThemePreset,
    engine: Engine,
) -> String {
    use merman_render::presentation::{HostTheme, Presentation};
    let engine = Presentation::new()
        .with_theme(HostTheme::from_preset(preset))
        .resolve()
        .materialize_engine(engine);
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn c4_rect_labeled<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    let text = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
        .unwrap();
    text.parent()
        .unwrap()
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap()
}

#[test]
fn c4_filled_nested_boundaries_paint_backgrounds_before_their_children() {
    let source = r##"C4Context
Boundary(outer, "Outer") {
  System(sibling, "Sibling")
  Boundary(inner, "Inner") {
    System(leaf, "Leaf")
  }
}
UpdateElementStyle(outer, $bgColor="#112233")
UpdateElementStyle(inner, $bgColor="#445566")
"##;
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let filled_boundary = |fill| {
        document
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(fill))
            .expect("filled boundary background")
    };
    let outer = filled_boundary("#112233");
    let inner = filled_boundary("#445566");
    let sibling = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-sibling"))
        .unwrap();
    let leaf = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-leaf"))
        .unwrap();
    assert!(
        outer.range().start < sibling.range().start,
        "outer fill must not cover its child"
    );
    assert!(outer.range().start < inner.range().start);
    assert!(
        inner.range().start < leaf.range().start,
        "inner fill must not cover its child"
    );
    assert!(
        leaf.range().start < c4_rect_labeled(&document, "Inner").range().start,
        "the inner outline and label remain above the child"
    );
}

#[test]
fn c4_source_relationship_paints_keep_shared_markers_without_host_paint() {
    let body = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for source_owned in [false, true] {
        let source = if source_owned {
            format!("{body}UpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n")
        } else {
            body.to_owned()
        };
        let config = if source_owned {
            serde_json::json!({})
        } else {
            serde_json::json!({"c4": {
                "relationship_text_color": "red", "relationship_line_color": "blue"
            }})
        };
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(config))
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let rendered = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_c4_shared_relationship_markers(&document);
        let line = document
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        assert_eq!(line.attribute("stroke"), Some("blue"));
        for label in ["Calls", "[HTTPS]"] {
            let text = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(text.attribute("fill"), Some("red"));
            if label == "[HTTPS]" {
                assert_eq!(text.attribute("font-style"), Some("italic"));
            }
        }
    }
}

fn assert_c4_shared_relationship_markers(document: &roxmltree::Document<'_>) {
    let line = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
        .unwrap();
    assert!(
        line.parent()
            .unwrap()
            .children()
            .all(|node| !node.has_tag_name("defs")),
        "source relationship colors must preserve Mermaid's relationship group"
    );
    for (attribute, suffix) in [
        ("marker-start", "-arrowend)"),
        ("marker-end", "-arrowhead)"),
    ] {
        assert!(line.attribute(attribute).unwrap().ends_with(suffix));
    }
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("marker"))
            .count(),
        4
    );
}

#[test]
fn c4_empty_and_typography_themes_keep_source_relationship_markers_shared() {
    use merman_render::presentation::{HostTheme, Presentation, ThemeRole};
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n";
    for theme in [
        HostTheme::new(),
        HostTheme::new().try_with_font_family("Inter").unwrap(),
        HostTheme::new()
            .try_with_role(ThemeRole::Text, "#123456")
            .unwrap(),
    ] {
        let parsed = Presentation::new()
            .with_theme(theme)
            .resolve()
            .materialize_engine(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let rendered = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_c4_shared_relationship_markers(&document);
    }
}

#[test]
fn c4_requested_host_line_paint_keeps_source_color_precedence() {
    use merman_render::presentation::{HostTheme, Presentation, ThemeRole};
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n";
    let theme = HostTheme::new()
        .try_with_role(ThemeRole::Line, "#123456")
        .unwrap();
    let parsed = Presentation::new()
        .with_theme(theme)
        .resolve()
        .materialize_engine(Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let rendered = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let line = document
        .descendants()
        .find(|node| node.has_tag_name("line"))
        .unwrap();
    assert_eq!(line.attribute("stroke"), Some("blue"));
    for attribute in ["marker-start", "marker-end"] {
        let id = line
            .attribute(attribute)
            .unwrap()
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let path = document
            .descendants()
            .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
            .unwrap()
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        assert_eq!(path.attribute("fill"), Some("blue"));
        assert_eq!(path.attribute("stroke"), Some("blue"));
    }
}

#[test]
fn c4_host_presets_reach_boundary_relation_and_marker_paint() {
    use merman_render::presentation::HostThemePreset;
    let source = "C4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for (preset, text, line, cluster_fill) in [
        (
            HostThemePreset::EditorLight,
            "#0f172a",
            "#64748b",
            "#f1f5f9",
        ),
        (HostThemePreset::EditorDark, "#e5e7eb", "#94a3b8", "#1e293b"),
    ] {
        let svg = render_c4_with_preset(source, preset, Engine::new());
        let document = roxmltree::Document::parse(&svg).unwrap();
        for label in ["Boundary", "Calls", "[HTTPS]"] {
            let node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(node.attribute("fill"), Some(text), "{preset:?} {label}");
        }
        for label in ["A", "B"] {
            let node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert!(
                node.attribute("style")
                    .unwrap()
                    .contains(&format!("fill:{text} !important")),
                "{preset:?} {label}"
            );
        }
        let boundary_background = document
            .descendants()
            .find(|node| node.attribute("class") == Some("c4-boundary-background"))
            .unwrap();
        assert_eq!(
            boundary_background.attribute("fill"),
            Some(cluster_fill),
            "{preset:?}"
        );
        let relation = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(relation.attribute("stroke"), Some(line), "{preset:?}");
        for marker in ["marker-start", "marker-end"] {
            let id = relation
                .attribute(marker)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .unwrap();
            let path = marker
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(path.attribute("fill"), Some(line), "{preset:?} marker fill");
            assert_eq!(
                path.attribute("stroke"),
                Some(line),
                "{preset:?} marker stroke"
            );
        }
    }
}

#[test]
fn c4_external_palette_retains_missing_family_defaults() {
    use merman_render::presentation::HostThemePreset;
    let source = "---\nconfig: {theme: base}\n---\nC4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\")\n";
    let svg = render_c4_with_preset(source, HostThemePreset::EditorDark, Engine::new());
    let document = roxmltree::Document::parse(&svg).unwrap();
    for label in ["Boundary", "Calls"] {
        let node = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
            .unwrap();
        assert_eq!(node.attribute("fill"), Some("#e5e7eb"));
    }
    assert_eq!(
        c4_rect_labeled(&document, "Boundary").attribute("fill"),
        Some("none")
    );
    let background = document
        .descendants()
        .find(|node| node.attribute("class") == Some("c4-boundary-background"))
        .unwrap();
    assert_eq!(background.attribute("fill"), Some("#1e293b"));
    let line = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
        .unwrap();
    assert_eq!(line.attribute("stroke"), Some("#94a3b8"));
}

#[test]
fn c4_host_relation_paint_preserves_explicit_source_and_site_owners() {
    use merman_render::presentation::HostThemePreset;
    let source = "C4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n";
    let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
        "c4": { "boundary_font_color": "green", "relationship_text_color": "orange", "relationship_line_color": "purple" }
    })));
    let svg = render_c4_with_preset(source, HostThemePreset::EditorDark, engine);
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (label, fill) in [("Boundary", "green"), ("Calls", "red"), ("[HTTPS]", "red")] {
        let node = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
            .unwrap();
        assert_eq!(node.attribute("fill"), Some(fill));
    }
    let relation = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
        .unwrap();
    assert_eq!(relation.attribute("stroke"), Some("blue"));
    let id = relation
        .attribute("marker-end")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let marker = document
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap();
    assert_eq!(
        marker
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap()
            .attribute("fill"),
        Some("blue")
    );
}

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[test]
fn c4_public_layout_and_svg_render_handle_deep_boundary_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.metadata().diagram_type, "c4");

    let options = LayoutOptions::default();
    let artifact = family::prepare(parsed, &options, session)
        .expect("layout should not depend on recursive boundary traversal");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    let c4: C4DiagramLayout = serde_json::from_value(projection["layout"]["C4Diagram"].clone())
        .expect("C4 layout projection");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");

    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("SVG painting should use an iterative boundary traversal")
        .svg()
        .to_owned();
    assert!(svg.contains(">Leaf</tspan>"));
    assert!(svg.contains(">B0</tspan>"));
    assert!(svg.contains(&format!(">B{}</tspan>", DEPTH - 1)));
}

#[test]
fn c4_unified_shapes_render_canonical_labels() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Person(person, "Person")
Container(system, "System", "Rust", "A short description")
Person_Ext(external, "External")
System(framed, "Framed", "A component-shaped system", $shape="component")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");

    for class in ["c4-name", "c4-type", "c4-descr"] {
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("g") && node.attribute("class") == Some(class) }),
            "missing unified C4 label section {class}: {svg}"
        );
    }
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();
    assert!(type_texts.iter().any(|text| text == "[Person]"));
    assert!(type_texts.iter().any(|text| text == "[Container: Rust]"));
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-descr"))
            .map(svg_text_content)
            .any(|text| text == "A short description")
    );
    assert_eq!(svg.matches("<circle ").count(), 2);
    assert_eq!(svg.matches("<polygon ").count(), 1);
    assert!(!svg.contains("<<person>>"));
    assert!(!svg.contains("<<system>>"));
    assert!(!svg.contains("<<external_person>>"));
}

#[test]
fn c4_uses_explicit_screen_available_width_without_changing_container_geometry() {
    let source = include_str!(
        "../../../fixtures/c4/upstream_docs_c4_c4_container_diagram_c4container_006.mmd"
    );
    let default = layout_c4_with_options(source, &LayoutOptions::default());
    let wide_screen = layout_c4_with_options(
        source,
        &LayoutOptions::default().with_screen_available_width(1280.0),
    );

    assert_eq!(default.container_width, 800.0);
    assert_eq!(default.screen_available_width, None);
    assert_eq!(wide_screen.container_width, 800.0);
    assert_eq!(wide_screen.screen_available_width, Some(1280.0));
    assert!(wide_screen.width > default.width);
    assert!(wide_screen.height < default.height);
}

#[test]
fn c4_svg_paints_each_boundary_subtree_in_mermaid_order() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(root_before, "Root Before")
Boundary(outer, "Outer Boundary") {
  System(outer_before, "Outer Before")
  Boundary(inner, "Inner Boundary") {
    System(inner_shape, "Inner Shape")
  }
  System(outer_after, "Outer After")
}
Boundary(sibling, "Sibling Boundary") {
  System(sibling_shape, "Sibling Shape")
}
System(root_after, "Root After")
Rel(inner_shape, root_before, "Leaves subtree")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .filter(|text| {
            matches!(
                text.as_str(),
                "Root Before"
                    | "Root After"
                    | "Outer Before"
                    | "Outer After"
                    | "Inner Shape"
                    | "Inner Boundary"
                    | "Outer Boundary"
                    | "Sibling Shape"
                    | "Sibling Boundary"
                    | "Leaves subtree"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        [
            "Root Before",
            "Root After",
            "Outer Before",
            "Outer After",
            "Inner Shape",
            "Inner Boundary",
            "Outer Boundary",
            "Sibling Shape",
            "Sibling Boundary",
            "Leaves subtree",
        ]
    );
}

#[test]
fn c4_relation_keeps_explicit_line_and_text_styles() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(a, "A")
System(b, "B")
Rel(a, b, "Calls", "HTTPS")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let calls = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Calls")
        .expect("relationship label");
    let technology = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "[HTTPS]")
        .expect("relationship technology label");
    let line = document
        .descendants()
        .find(|node| {
            matches!(node.tag_name().name(), "line" | "path")
                && node.attribute("stroke") == Some("blue")
                && node.attribute("marker-end").is_some()
        })
        .expect("relationship line");

    assert_eq!(calls.attribute("fill"), Some("red"));
    assert!(calls.attribute("style").is_some_and(|style| {
        style.contains("text-anchor: middle") && style.contains("font-family:")
    }));
    assert_eq!(technology.attribute("fill"), Some("red"));
    assert_eq!(technology.attribute("font-style"), Some("italic"));
    assert_eq!(line.attribute("stroke-width"), Some("1"));
    assert_eq!(line.attribute("style"), Some("fill: none;"));
}

#[test]
fn c4_relationships_resolve_boundary_endpoints_and_prefer_shapes() {
    let source = r#"C4Context
System(outside, "Outside")
Boundary(left, "Left") {
  System(inside, "Inside")
}
Boundary(right, "Right") {
  System(other, "Other")
}
Rel(outside, left, "Enters")
Rel(left, outside, "Leaves")
Rel(left, right, "Connects")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::headless_svg_defaults());
    assert_eq!(layout.rels.len(), 3);
    for (alias, point) in [
        ("left", &layout.rels[0].end_point),
        ("left", &layout.rels[1].start_point),
        ("left", &layout.rels[2].start_point),
        ("right", &layout.rels[2].end_point),
    ] {
        let boundary = layout.boundaries.iter().find(|b| b.alias == alias).unwrap();
        let on_vertical = (point.x - boundary.x).abs() < 1e-8
            || (point.x - boundary.x - boundary.width).abs() < 1e-8;
        let on_horizontal = (point.y - boundary.y).abs() < 1e-8
            || (point.y - boundary.y - boundary.height).abs() < 1e-8;
        assert!(
            on_vertical || on_horizontal,
            "endpoint must meet {alias}'s perimeter"
        );
        assert!(point.x >= boundary.x - 1e-8 && point.x <= boundary.x + boundary.width + 1e-8);
        assert!(point.y >= boundary.y - 1e-8 && point.y <= boundary.y + boundary.height + 1e-8);
    }
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    assert!(svg.contains("Enters") && svg.contains("Leaves") && svg.contains("Connects"));

    let shared_alias = source
        .replace("System(outside,", "System(left,")
        .replace("outside, left", "left, inside")
        .replace("left, outside", "inside, left");
    let layout = layout_c4_with_options(&shared_alias, &LayoutOptions::headless_svg_defaults());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "left")
        .unwrap();
    let endpoint = &layout.rels[0].start_point;
    assert!(endpoint.x >= shape.x - 1e-8 && endpoint.x <= shape.x + shape.width + 1e-8);
    assert!(endpoint.y >= shape.y - 1e-8 && endpoint.y <= shape.y + shape.height + 1e-8);
}

#[test]
fn c4_dynamic_text_colors_remain_single_xml_attributes_for_site_and_source_values() {
    let site_color = r#"red" data-probe="site&raw"#;
    let authored_color = "blue' data-probe='source&raw";
    let source = format!(
        r#"C4Context
Boundary(boundary, "Boundary") {{
  System(a, "A")
}}
System(b, "B")
Rel(a, b, "Calls", "HTTPS")
UpdateRelStyle(a, b, $textColor="{authored_color}")
"#
    );
    let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
        serde_json::json!({"c4": {
            "boundary_font_color": site_color,
            "relationship_text_color": site_color
        }}),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    let document =
        roxmltree::Document::parse(&svg).expect("dynamic color values must not break XML");
    for (content, expected) in [
        ("Boundary", site_color),
        ("Calls", authored_color),
        ("[HTTPS]", authored_color),
    ] {
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == content)
            .unwrap();
        assert_eq!(text.attribute("fill"), Some(expected), "{content}");
    }
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-probe").is_none())
    );
}

use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::VennDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_typed_venn(input: &str) -> (VennDiagramLayout, String) {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.metadata().diagram_type, "venn");

    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Venn layout");
    let layout: VennDiagramLayout =
        serde_json::from_value(projection["layout"]["VennDiagram"].clone())
            .expect("Venn layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("venn-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render SVG")
        .svg()
        .to_owned();

    (layout, svg)
}

fn find_venn_area<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    sets: &str,
    class: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-venn-sets") == Some(sets)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|item| item == class))
        })
        .unwrap_or_else(|| panic!("Venn area `{sets}` with class `{class}`"))
}

#[test]
fn venn_typed_render_model_outputs_classic_svg_structure() {
    let input = r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
"##;

    let (layout, svg) = render_typed_venn(input);
    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 800 450""#));
    assert!(svg.contains(r#"<text class="venn-title""#));
    assert!(svg.contains(">Product Surface</text>"));
    assert!(svg.contains(r#"<g transform="translate(0, 24)">"#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-1""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
    assert!(svg.contains(r#"data-venn-sets="A_B""#));
    assert!(svg.contains(">Core</tspan></text>"));
    assert!(svg.contains(">Shared</tspan></text>"));
}

#[test]
fn venn_styles_and_text_nodes_render_inline_overrides() {
    let input = r##"%%{init: {"venn": {"useDebugLayout": true}, "themeVariables": {"vennSetTextColor": "#222222"}}}%%
venn-beta
set A["Frontend"]:20
  text A1["React"]
set B["Backend"]:16
union A,B["API"]:5
  text AB1["OpenAPI"]
style A fill:#ff6b6b, color:#101010, stroke:#202020, stroke-width:7, fill-opacity:0.42
style A,B fill:#00ffcc, color:#003333
style A1 color:#123456
"##;

    let (_layout, svg) = render_typed_venn(input);

    assert!(svg.contains(r#"style="fill: #ff6b6b; fill-opacity: 0.42; stroke: #202020; stroke-width: 7; stroke-opacity: 0.95;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #101010;""#));
    assert!(svg.contains(r#"style="fill-opacity: 1; fill: #00ffcc;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #003333;""#));
    assert!(svg.contains(r#"<g class="venn-text-nodes">"#));
    assert!(svg.contains(r#"<g class="venn-text-area" font-size="20px">"#));
    assert!(svg.contains(r#"class="venn-text-debug-circle""#));
    assert!(svg.contains(r#"class="venn-text-debug-cell""#));
    assert!(svg.contains(r#"<foreignObject class="venn-text-node-fo""#));
    assert!(svg.contains(r#"<span xmlns="http://www.w3.org/1999/xhtml" class="venn-text-node""#));
    assert!(svg.contains("color: #123456;\">React</span>"));
}

#[test]
fn venn_canonical_typed_path_renders_svg() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r##"%%{init: {"venn": {"useMaxWidth": false, "width": 640, "height": 360}}}%%
venn-beta
set A
set B
union A,B
"##,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.metadata().diagram_type, "venn");

    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    assert_eq!(
        artifact.family_kind(),
        merman_render::family::RenderFamilyKind::Venn
    );
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 640 360""#));
    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"height="360""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
}

#[test]
fn venn_typed_layout_and_canonical_renderer_agree() {
    let input = r##"venn-beta
set A
set B
union A,B
"##;

    let (layout, svg) = render_typed_venn(input);

    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
}

#[test]
fn venn_repeated_union_members_keep_upstream_constraints_and_finite_geometry() {
    let input = r##"venn-beta
set A["Alpha"]:20
set B["Beta"]:12
union A,A,B["Repeated"]:3
"##;

    let (layout, svg) = render_typed_venn(input);
    let (repeated_layout, repeated_svg) = render_typed_venn(input);

    // Mermaid 11.16 enumerates member positions, then de-duplicates pair keys. Therefore A,A,B
    // contributes exactly the A,A self-pair and the A,B pair to the layout-only constraints.
    assert_eq!(
        layout
            .areas
            .iter()
            .map(|area| (area.sets.join("|"), area.size))
            .collect::<Vec<_>>(),
        [
            ("A".to_string(), 20.0),
            ("B".to_string(), 12.0),
            ("A|A|B".to_string(), 3.0),
            ("A|A".to_string(), 5.0),
            ("A|B".to_string(), 3.0),
        ]
    );
    assert_eq!(
        serde_json::to_value(&layout).expect("serialize first typed layout"),
        serde_json::to_value(&repeated_layout).expect("serialize repeated typed layout"),
        "the source-backed Venn optimizer must remain deterministic"
    );
    assert_eq!(svg, repeated_svg);

    for area in &layout.areas {
        assert!(area.size.is_finite(), "non-finite size for {:?}", area.sets);
        assert!(
            area.text_x.is_finite() && area.text_y.is_finite(),
            "non-finite label position for {:?}",
            area.sets
        );
        assert!(!area.path.is_empty(), "missing path for {:?}", area.sets);
        assert!(
            !area.path.contains("NaN") && !area.path.contains("inf"),
            "non-finite path for {:?}: {}",
            area.sets,
            area.path
        );
        assert_eq!(area.circles.len(), area.sets.len());
        for circle in &area.circles {
            assert!(
                circle.x.is_finite()
                    && circle.y.is_finite()
                    && circle.radius.is_finite()
                    && circle.radius > 0.0,
                "non-finite circle for {:?}: {circle:?}",
                area.sets
            );
        }
    }

    let document = roxmltree::Document::parse(&svg).expect("valid repeated-member Venn SVG");
    let self_pair = find_venn_area(&document, "A_A", "venn-intersection");
    let repeated_union = find_venn_area(&document, "A_A_B", "venn-intersection");
    for area in [self_pair, repeated_union] {
        let path = area
            .children()
            .find(|child| child.has_tag_name("path"))
            .and_then(|path| path.attribute("d"))
            .expect("rendered intersection path");
        assert!(!path.is_empty());
        assert!(!path.contains("NaN") && !path.contains("inf"));
    }
    assert_eq!(svg.matches(">Repeated</tspan></text>").count(), 1);
}

#[test]
fn venn_hand_drawn_replaces_styled_areas_with_seeded_rough_paths() {
    let input = r##"---
config:
  look: handDrawn
  handDrawnSeed: 1
---
venn-beta
set A
set B
union A,B
style A fill:#ff6b6b,stroke:#202020,stroke-width:7
style A,B fill:#ffe66d,color:#003333
"##;

    let (_, svg) = render_typed_venn(input);
    let document = roxmltree::Document::parse(&svg).expect("valid Venn SVG");
    let circle_a = find_venn_area(&document, "A", "venn-circle");
    let circle_a_children: Vec<_> = circle_a
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(circle_a_children.len(), 2);
    assert!(circle_a_children[0].has_tag_name("g"));
    assert!(circle_a_children[1].has_tag_name("text"));

    let circle_a_paths: Vec<_> = circle_a_children[0]
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(circle_a_paths.len(), 2);
    assert!(circle_a_paths.iter().all(|path| path.has_tag_name("path")));
    assert_eq!(
        circle_a_paths[0].attribute("stroke"),
        Some("rgba(255, 107, 107, 0.3)")
    );
    assert_eq!(circle_a_paths[0].attribute("stroke-width"), Some("2"));
    assert_eq!(circle_a_paths[0].attribute("fill"), Some("none"));
    assert_eq!(circle_a_paths[1].attribute("stroke"), Some("#202020"));
    assert_eq!(circle_a_paths[1].attribute("stroke-width"), Some("7"));
    assert_eq!(circle_a_paths[1].attribute("fill"), Some("none"));

    let circle_b = find_venn_area(&document, "B", "venn-circle");
    let circle_b_fill_d = circle_b
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .and_then(|path| path.attribute("d"))
        .expect("set B rough fill path");
    assert_ne!(circle_a_paths[0].attribute("d"), Some(circle_b_fill_d));

    let intersection = find_venn_area(&document, "A_B", "venn-intersection");
    let intersection_children: Vec<_> = intersection
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(intersection_children.len(), 2);
    assert!(intersection_children[0].has_tag_name("g"));
    assert!(intersection_children[1].has_tag_name("text"));
    let intersection_paths: Vec<_> = intersection_children[0]
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(intersection_paths.len(), 1);
    assert_eq!(
        intersection_paths[0].attribute("stroke"),
        Some("rgba(255, 230, 109, 0.7)")
    );
    assert_eq!(intersection_paths[0].attribute("stroke-width"), Some("2"));
    assert_eq!(intersection_paths[0].attribute("fill"), Some("none"));

    let (_, repeated_svg) = render_typed_venn(input);
    assert_eq!(svg, repeated_svg);

    let (_, other_seed_svg) =
        render_typed_venn(&input.replace("handDrawnSeed: 1", "handDrawnSeed: 2"));
    let other_seed_document =
        roxmltree::Document::parse(&other_seed_svg).expect("valid Venn SVG for another seed");
    let other_seed_circle_a_fill_d = find_venn_area(&other_seed_document, "A", "venn-circle")
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .and_then(|path| path.attribute("d"))
        .expect("set A rough fill path for another seed");
    assert_ne!(
        circle_a_paths[0].attribute("d"),
        Some(other_seed_circle_a_fill_d)
    );
}

#[test]
fn venn_hand_drawn_keeps_unstyled_intersection_path_transparent() {
    let input = r##"---
config:
  look: handDrawn
  handDrawnSeed: 1
---
venn-beta
set A
set B
union A,B
"##;

    let (_, svg) = render_typed_venn(input);
    let document = roxmltree::Document::parse(&svg).expect("valid Venn SVG");
    let circle_a = find_venn_area(&document, "A", "venn-circle");
    let circle_a_fill = circle_a
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .expect("set A rough fill path");
    assert_eq!(
        circle_a_fill.attribute("stroke"),
        Some("hsla(240, 100%, 66.2745098039%, 0.30000000000000004)")
    );

    let intersection = find_venn_area(&document, "A_B", "venn-intersection");
    let children: Vec<_> = intersection
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();

    assert_eq!(children.len(), 2);
    assert!(children[0].has_tag_name("path"));
    assert_eq!(children[0].attribute("style"), Some("fill-opacity: 0;"));
    assert!(children[1].has_tag_name("text"));
}

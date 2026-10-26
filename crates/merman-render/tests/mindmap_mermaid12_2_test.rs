#![cfg(feature = "diagram-mindmap")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::environment::TextMeasurementPhase;
use merman_render::family;
use merman_render::model::MindmapDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextStyle, WrapMode};

fn label_translation(document: &roxmltree::Document<'_>, id: &str) -> (f64, f64) {
    let group = document
        .descendants()
        .find(|element| element.attribute("id") == Some(id))
        .unwrap();
    let label = group
        .children()
        .find(|element| element.attribute("class") == Some("label"))
        .unwrap();
    let (x, y) = label
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    (x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn measure_visible_svg_text(source: &str, visible: &str) -> merman_render::text::TextMetrics {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let config = parsed.metadata().effective_config.as_value();
    let font_family = config
        .pointer("/themeVariables/fontFamily")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let style = TextStyle {
        font_family,
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    session
        .text_measurer(TextMeasurementPhase::SvgBBox)
        .measure_wrapped(visible, &style, None, WrapMode::SvgLike)
}

fn render(source: &str, diagram_id: &str) -> (String, MindmapDiagramLayout) {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["MindmapDiagram"].clone()).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();
    (svg, layout)
}

#[test]
fn mindmap_mermaid_12_2_svg_labels_center_every_shape_without_text_anchor() {
    // mindmapDb.getData.test.ts issue #8387: SVG text must be offset by half its width.
    let (svg, layout) = render(
        r#"---
config:
  layout: tidy-tree
  htmlLabels: false
---
mindmap
Root((Centered))
  square[Centered]
  rounded(Centered)
  hexagon{{Centered}}
  cloud)Centered(
  bang))Centered((
  Centered
"#,
        "mindmap-12-2-center",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(!svg.contains("<foreignObject"));
    assert_eq!(layout.nodes.len(), 7);
    // edges.js insertEdgeLabel moves createText's returned empty <text> into .label;
    // createFormattedText's unused background group remains under .edgeLabels.
    let edge_labels = document
        .descendants()
        .find(|element| element.attribute("class") == Some("edgeLabels"))
        .unwrap();
    let edge_children = edge_labels
        .children()
        .filter(roxmltree::Node::is_element)
        .collect::<Vec<_>>();
    assert_eq!(edge_children.len(), 12);
    for pair in edge_children.chunks_exact(2) {
        assert_eq!(pair[0].attribute("class"), Some("edgeLabel"));
        let label = pair[0]
            .children()
            .find(|element| element.is_element())
            .unwrap();
        assert_eq!(label.attribute("class"), Some("label"));
        let text = label
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        assert_eq!(text.len(), 1);
        assert!(text[0].has_tag_name("text"));
        assert!(pair[1].attribute("class").is_none());
        let background = pair[1]
            .children()
            .find(|element| element.is_element())
            .unwrap();
        assert!(background.has_tag_name("rect"));
        assert_eq!(background.attribute("class"), Some("background"));
    }
    let measured = measure_visible_svg_text("mindmap\nCentered\n", "Centered");
    for node in &layout.nodes {
        let dom_id = format!("mindmap-12-2-center-node_{}", node.id);
        let group = document
            .descendants()
            .find(|element| element.attribute("id") == Some(dom_id.as_str()))
            .unwrap();
        let label = group
            .children()
            .find(|element| element.attribute("class") == Some("label"))
            .unwrap();
        let transform = label.attribute("transform").unwrap();
        let (x, y) = transform
            .strip_prefix("translate(")
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .split_once(',')
            .unwrap();
        let x: f64 = x.trim().parse().unwrap();
        let y: f64 = y.trim().parse().unwrap();
        assert!(
            (x + measured.width / 2.0).abs() < 0.01,
            "{dom_id}: {transform}"
        );
        assert!(
            (y + measured.height / 2.0).abs() < 0.01,
            "{dom_id}: {transform}"
        );
        let text = label
            .descendants()
            .find(|element| element.has_tag_name("text"))
            .unwrap();
        assert!(text.attribute("text-anchor").is_none());
    }
    let (formatted_svg, _) = render(
        "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n---\nmindmap\nRoot[**Square** *italic*]\n",
        "mindmap-formatted",
    );
    let formatted = roxmltree::Document::parse(&formatted_svg).unwrap();
    assert!(formatted.descendants().any(|element| {
        element.has_tag_name("tspan") && element.attribute("font-weight") == Some("bold")
    }));
    assert!(formatted.descendants().any(|element| {
        element.has_tag_name("tspan") && element.attribute("font-style") == Some("italic")
    }));
}

#[test]
fn mindmap_mermaid_12_2_depth_sections_reach_node_and_edge_svg_classes() {
    let (svg, _) = render(
        r#"---
config:
  layout: tidy-tree
  mindmap:
    colorBy: depth
---
mindmap
    root((Root))
          a[Branch A]
              leafA[Leaf A]
          b[Branch B]
            leafB[Leaf B]
"#,
        "mindmap-12-2-depth",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, class) in [
        ("node_0", "section-root"),
        ("node_1", "section-0"),
        ("node_2", "section-1"),
        ("node_3", "section-0"),
        ("node_4", "section-1"),
        ("edge_0_1", "section-edge-0"),
        ("edge_1_2", "section-edge-1"),
        ("edge_0_3", "section-edge-0"),
        ("edge_3_4", "section-edge-1"),
    ] {
        let dom_id = format!("mindmap-12-2-depth-{id}");
        let element = document
            .descendants()
            .find(|element| element.attribute("id") == Some(dom_id.as_str()))
            .unwrap();
        assert!(
            element
                .attribute("class")
                .unwrap()
                .split_whitespace()
                .any(|token| token == class)
        );
    }
}

#[test]
fn mindmap_mermaid_12_2_circle_encloses_wrapped_labels_with_one_neo_padding_step() {
    for html_labels in [false, true] {
        let mut classic_radius = None;
        let mut classic_label = None;
        for look in ["classic", "neo"] {
            let source = format!(
                "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: {html_labels}\n  look: {look}\n  mindmap:\n    padding: 7\n    maxNodeWidth: 50\n---\nmindmap\nRoot((A tall wrapping circle label))\n"
            );
            let (svg, layout) = render(&source, "mindmap-12-2-circle");
            let node = &layout.nodes[0];
            assert_eq!(node.width, node.height);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let circle = document
                .descendants()
                .find(|element| element.has_tag_name("circle"))
                .unwrap();
            let radius: f64 = circle.attribute("r").unwrap().parse().unwrap();
            assert!((radius - node.width / 2.0).abs() < 0.001);
            let label = label_translation(&document, "mindmap-12-2-circle-node_0");
            // The 12.2 browser oracle preserves the label across looks. circle.ts adds full
            // configured padding (7) for classic and exactly one 16px step for Neo: +9px radius.
            if look == "classic" {
                classic_radius = Some(radius);
                classic_label = Some(label);
            } else {
                assert!((radius - classic_radius.unwrap() - 9.0).abs() < 0.001);
                assert_eq!(Some(label), classic_label);
            }
        }
    }
}

#[test]
fn mindmap_mermaid_12_2_rect_render_padding_preserves_other_shape_configuration() {
    // mindmapRenderer.ts fixes rect padding at 10; squareRect.ts/drawRect.ts give an empty
    // classic rect a 40x20 box and a Neo rect a 32x24 box, regardless of mindmap.padding.
    // Hexagon retains its DB padding, and mindmapCircle forwards the configured full padding.
    for (look, empty_dimensions) in [("classic", (40.0, 20.0)), ("neo", (32.0, 24.0))] {
        let mut previous = None::<MindmapDiagramLayout>;
        for padding in [7, 31] {
            let source = format!(
                "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n  look: {look}\n  mindmap:\n    padding: {padding}\n---\nmindmap\nRoot((Circle))\n  rect[Rectangle]\n  empty[\"`![ignored](#unused)`\"]\n  hexagon{{{{Hexagon}}}}\n"
            );
            let (svg, layout) = render(&source, "mindmap-12-2-padding");
            let empty = &layout.nodes[2];
            assert_eq!((empty.width, empty.height), empty_dimensions);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                label_translation(&document, "mindmap-12-2-padding-node_2"),
                (0.0, 0.0)
            );
            let empty_group = document
                .descendants()
                .find(|node| node.attribute("id") == Some("mindmap-12-2-padding-node_2"))
                .unwrap();
            let rect = empty_group
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert_eq!(
                (
                    rect.attribute("width").unwrap().parse::<f64>().unwrap(),
                    rect.attribute("height").unwrap().parse::<f64>().unwrap(),
                ),
                empty_dimensions
            );
            if let Some(previous) = previous {
                assert_eq!(layout.nodes[1].width, previous.nodes[1].width);
                assert_eq!(layout.nodes[1].height, previous.nodes[1].height);
                if look == "classic" {
                    assert!(layout.nodes[3].height > previous.nodes[3].height);
                    assert!(layout.nodes[0].width > previous.nodes[0].width);
                } else {
                    assert_eq!(layout.nodes[0].width, previous.nodes[0].width);
                }
            }
            previous = Some(layout);
        }
    }
}

#[test]
fn mindmap_svg_entity_spelling_matches_measured_create_text_content() {
    for (entity, visible) in [
        ("#9829;", "&#9829;"),
        // encodeEntities also rewrites the hash in authored &#...;, keeping its leading ampersand.
        // A 12.2 browser DOM probe therefore observes a doubled ampersand for this spelling.
        ("&#9829;", "&&#9829;"),
        ("#nbsp;", "&nbsp;"),
        ("&nbsp;", "&nbsp;"),
        ("#quot;", "&quot;"),
        ("&#160;", "&&#160;"),
        ("#lt;", "<"),
        ("&amp;", "&"),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n---\nmindmap\nRoot[{entity}]\n"
        );
        let (svg, _) = render(&source, "mindmap-12-2-entity");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label_text: String = document
            .descendants()
            .filter(|element| {
                element.has_tag_name("tspan")
                    && element.attribute("class") == Some("text-inner-tspan")
            })
            .filter_map(|element| element.text())
            .collect();
        assert_eq!(label_text, visible, "{entity}: {svg}");
        assert!(!svg.contains('ﬂ') && !svg.contains('¶'), "{entity}: {svg}");

        // Compare with the rendered DOM's literal text in the same measurement profile.
        // This detects measuring a Unicode entity expansion while emitting its spelling.
        let measured = measure_visible_svg_text(&source, visible);
        let (x, y) = label_translation(&document, "mindmap-12-2-entity-node_0");
        assert!((-2.0 * x - measured.width).abs() < 0.02, "{entity}");
        assert!((-2.0 * y - measured.height).abs() < 0.001, "{entity}");
    }
}

#[test]
fn mindmap_svg_markdown_images_do_not_allocate_html_image_boxes() {
    for (label, visible) in [
        ("![alt](image.png)", ""),
        ("Before ![alt](image.png) after", "Before after"),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n---\nmindmap\nRoot[\"`{label}`\"]\n"
        );
        let (svg, _) = render(&source, "mindmap-12-2-image");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let emitted: String = document
            .descendants()
            .filter(|element| {
                element.has_tag_name("tspan")
                    && element.attribute("class") == Some("text-inner-tspan")
            })
            .filter_map(|element| element.text())
            .collect();
        assert_eq!(emitted, visible, "{label}: {svg}");
        assert!(!svg.contains("<image") && !svg.contains("<foreignObject"));
        let label = label_translation(&document, "mindmap-12-2-image-node_0");
        if visible.is_empty() {
            assert_eq!(label, (0.0, 0.0));
        } else {
            let literal_source = format!(
                "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: false\n---\nmindmap\nRoot[{visible}]\n"
            );
            let (literal_svg, _) = render(&literal_source, "mindmap-12-2-image-literal");
            let literal = roxmltree::Document::parse(&literal_svg).unwrap();
            assert_eq!(
                label,
                label_translation(&literal, "mindmap-12-2-image-literal-node_0")
            );
        }
    }
}

#[test]
fn mindmap_svg_family_local_fixture_preserves_browser_words_styles_and_font_priority() {
    const ID: &str = "local_mermaid_12_2_svg_markdown_font_entities";
    const SOURCE: &str = include_str!(
        "../../../fixtures/_family_local/mermaid-12-2/mindmap/local_mermaid_12_2_svg_markdown_font_entities.mmd"
    );
    const UPSTREAM: &str = include_str!(
        "../../../fixtures/_family_local/mermaid-12-2/mindmap/local_mermaid_12_2_svg_markdown_font_entities.upstream.svg"
    );
    let (svg, layout) = render(SOURCE, ID);
    let local = roxmltree::Document::parse(&svg).unwrap();
    let upstream = roxmltree::Document::parse(UPSTREAM).unwrap();
    assert_eq!(layout.nodes.len(), 5);
    assert_eq!(layout.edges.len(), 4);
    for document in [&local, &upstream] {
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(css.contains("font-family:Arial;"));
        assert!(css.contains("font-size:24px;"));
        assert!(
            !document
                .descendants()
                .any(|node| { node.has_tag_name("foreignObject") || node.has_tag_name("image") })
        );
    }
    let styled_characters = |document: &roxmltree::Document<'_>, node_id: &str| {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(node_id))
            .unwrap();
        node.descendants()
            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
            .flat_map(|node| {
                let bold = node.attribute("font-weight") == Some("bold");
                let italic = node.attribute("font-style") == Some("italic");
                // Browser and deterministic profiles split words into different rows. Compare
                // the authored characters and emphasis independently of those wrap separators.
                node.text()
                    .unwrap_or_default()
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .map(move |ch| (ch, bold, italic))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    for (index, expected) in [
        "Awrappedcirclelabelwithemphasis",
        "",
        "Beforeaftertail",
        "&#9829;&&#9829;&nbsp;&nbsp;&<",
        "Firstlinesecondline",
    ]
    .into_iter()
    .enumerate()
    {
        let node_id = format!("{ID}-node_{index}");
        let actual = styled_characters(&local, &node_id);
        assert_eq!(actual, styled_characters(&upstream, &node_id), "{node_id}");
        assert_eq!(
            actual.iter().map(|(ch, _, _)| ch).collect::<String>(),
            expected,
            "{node_id}"
        );
        if index > 0 {
            let (x, y) = label_translation(&local, &node_id);
            let (upstream_x, upstream_y) = label_translation(&upstream, &node_id);
            let rect_dimensions = |document: &roxmltree::Document<'_>| {
                let group = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(node_id.as_str()))
                    .unwrap();
                let rect = group
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                (
                    rect.attribute("width").unwrap().parse::<f64>().unwrap(),
                    rect.attribute("height").unwrap().parse::<f64>().unwrap(),
                )
            };
            let (width, height) = rect_dimensions(&local);
            let (upstream_width, upstream_height) = rect_dimensions(&upstream);
            assert!((width + 2.0 * x - upstream_width - 2.0 * upstream_x).abs() < 0.001);
            assert!((height + 2.0 * y - upstream_height - 2.0 * upstream_y).abs() < 0.001);
        }
    }
    assert_eq!(
        label_translation(&local, &format!("{ID}-node_1")),
        (0.0, 0.0)
    );
    let (_, smaller_font) = render(&SOURCE.replace("fontSize: '24px'", "fontSize: '12px'"), ID);
    assert!(layout.nodes[0].height > smaller_font.nodes[0].height);
}

#[test]
fn mindmap_mermaid_12_2_neo_html_labels_follow_theme_palette_colors() {
    // styles.ts gives Neo text and HTML spans the same fill/color for every section.
    for (theme, root_color, child_color) in [
        ("default", "#123456", "#234567"),
        ("neutral", "#234567", "#234567"),
        ("redux", "#345678", "#345678"),
        ("redux-dark", "#345678", "#345678"),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: tidy-tree\n  look: neo\n  theme: {theme}\n  themeVariables:\n    cScaleLabel0: '#123456'\n    cScaleLabel1: '#234567'\n    nodeBorder: '#345678'\n    gitBranchLabel0: '#456789'\n---\nmindmap\nRoot\n  Child\n"
        );
        let (svg, _) = render(&source, "mindmap-12-2-neo");
        assert!(svg.contains("<span class=\"nodeLabel markdown-node-label\"><p>Root</p></span>"));
        let document = roxmltree::Document::parse(&svg).expect("valid escaped Mindmap CSS XML");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Mindmap stylesheet");
        assert!(css.contains(&format!(
            "#mindmap-12-2-neo [data-look=\"neo\"].mindmap-node.section-root .text-inner-tspan,#mindmap-12-2-neo [data-look=\"neo\"].mindmap-node.section-root span{{fill:{root_color};color:{root_color};}}"
        )), "theme={theme}: {svg}");
        assert!(css.contains(&format!(
            "#mindmap-12-2-neo [data-look=\"neo\"].mindmap-node.section-0 text,#mindmap-12-2-neo [data-look=\"neo\"].mindmap-node.section-0 span{{fill:{child_color};color:{child_color};}}"
        )), "theme={theme}: {svg}");
    }
}

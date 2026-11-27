#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

use kurbo::{BezPath, Point, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgPipeline;
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

fn render(source: &str, canvas: Color, resvg: bool) -> String {
    render_with_theme(source, canvas, resvg, None)
}

fn render_with_theme(
    source: &str,
    canvas: Color,
    resvg: bool,
    theme: Option<NativeTheme>,
) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap()
    .with_canvas_background(canvas);
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    if resvg {
        rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .unwrap()
            .svg()
            .as_str()
            .to_owned()
    } else {
        rendered.svg().to_owned()
    }
}

fn text_content(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

#[test]
fn shoal_non_html_labels_decode_once_and_source_css_stays_css() {
    let source = r##"---
config:
  layout: shoal
  htmlLabels: false
  flowchart:
    wrappingWidth: 640
---
flowchart LR
  subgraph G["Title #quot;abc#quot; #333; &lt;group&gt;"]
    A["Text #quot;abc#quot; #333; &lt;tag&gt; &amp;lt;keep&amp;gt;"] e@-->|"url(#evil) #quot;edge#quot; #333;"| B["&lt;/text&gt;&lt;script&gt;literal&lt;/script&gt;"]
  end
  style G fill:#333,stroke:#345,color:#fff,font-family:"a;b"
  style B fill:#FEF2F2,stroke:#DC2626,color:#991B1B
  linkStyle 0 stroke:#0EA5E9,stroke-width:2px,font-family:"a;b"
"##;
    let svg = render(source, Color::rgb(255, 255, 255), false);
    let doc = Document::parse(&svg).unwrap();
    let labels = doc
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(text_content)
        .collect::<Vec<_>>();
    for expected in [
        "Title \"abc\" ō <group>",
        "Text \"abc\" ō <tag> &lt;keep&gt;",
        "url(#evil) \"edge\" ō",
        "</text><script>literal</script>",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "{expected:?} missing from {labels:?}"
        );
    }
    assert!(!doc.descendants().any(|node| node.has_tag_name("script")));
    let literal_text = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("text") && text_content(*node) == "</text><script>literal</script>"
        })
        .unwrap();
    assert!(
        literal_text
            .attribute("style")
            .is_some_and(|style| style.contains("fill:#991B1B !important"))
    );
    assert!(!doc.descendants().any(|node| {
        node.is_element()
            && node
                .attributes()
                .any(|attribute| attribute.value().contains("url(#evil)"))
    }));
    let cluster_style = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node
                    .parent()
                    .is_some_and(|parent| parent.attribute("data-id") == Some("G"))
        })
        .unwrap()
        .attribute("style")
        .unwrap();
    assert!(
        cluster_style.contains("fill:#333 !important;stroke:#345 !important"),
        "{cluster_style}"
    );
    let edge_style = doc
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap()
        .attribute("style")
        .unwrap();
    assert_eq!(
        edge_style.matches("stroke:#0EA5E9").count(),
        1,
        "{edge_style}"
    );
    assert!(edge_style.contains("font-family:\"a;b\""), "{edge_style}");
    assert!(!edge_style.contains(";;;"), "{edge_style}");
}

#[test]
fn shoal_svg_preserves_bare_angle_text_and_only_authored_breaks_make_rows() {
    let source = r##"---
config:
  layout: shoal
  htmlLabels: false
  flowchart:
    wrappingWidth: 640
---
flowchart LR
  subgraph G["Container <group>"]
    A["A & B <tag> #quot;quoted#quot; #333;<BR />Second &lt;br&gt; &amp;lt;"] e@-->|"<edge>"| B["</text><script>literal</script>"]
  end
"##;
    for resvg in [false, true] {
        let svg = render(source, Color::TRANSPARENT, resvg);
        let doc = Document::parse(&svg).unwrap();
        let text = doc
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        for expected in [
            "Container <group>",
            "<edge>",
            "</text><script>literal</script>",
        ] {
            assert!(
                text.iter().any(|node| text_content(*node) == expected),
                "missing {expected}"
            );
        }
        let multiline = text
            .iter()
            .find(|node| text_content(**node).starts_with("A & B"))
            .unwrap();
        assert_eq!(
            multiline
                .children()
                .filter(|node| node.has_tag_name("tspan"))
                .map(text_content)
                .collect::<Vec<_>>(),
            ["A & B <tag> \"quoted\" ō", "Second <br> &lt;"]
        );
        assert!(!doc.descendants().any(|node| {
            ["tag", "edge", "script"]
                .iter()
                .any(|name| node.has_tag_name(*name))
        }));
        assert!(
            svg.contains("-label-cutout"),
            "the recovered edge label is measured"
        );
    }
}

#[test]
fn shoal_text_projection_respects_the_backend_and_effective_label_mode() {
    for (backend, config, node_literal, edge_literal) in [
        ("shoal", "htmlLabels: false", true, true),
        ("shoal", "htmlLabels: true", false, false),
        ("shoal", "flowchart:\n    htmlLabels: false", false, true),
        #[cfg(feature = "layout-dagre")]
        ("dagre", "htmlLabels: false", false, false),
        #[cfg(feature = "layout-elk")]
        ("elk", "htmlLabels: false", false, false),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: {backend}\n  {config}\n---\n\
             flowchart LR\nsubgraph G[\"Group <title-token>\"]\n\
             A[\"Node <node-token>\"] -->|\"Edge <edge-token>\"| B\nend\n"
        );
        let svg = render(&source, Color::TRANSPARENT, false);
        let doc = Document::parse(&svg).unwrap();
        let text = text_content(doc.root_element());
        for (token, expected) in [
            ("<node-token>", node_literal),
            ("<edge-token>", edge_literal),
            ("<title-token>", edge_literal),
        ] {
            assert_eq!(
                text.contains(token),
                expected,
                "{backend}: {config}: {token}"
            );
        }
    }
}

#[test]
fn shoal_transparent_edge_labels_cut_their_shaft_in_svg_and_resvg() {
    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  htmlLabels: {html_labels}\n---\n\
             flowchart LR\nA e@-->|Caption| B\n"
        );
        for resvg in [false, true] {
            let svg = render(&source, Color::TRANSPARENT, resvg);
            assert_label_cutout(&svg);
        }
        assert!(!render(&source, Color::rgb(255, 255, 255), false).contains("-label-cutout"));
    }
}

#[test]
fn shoal_explicitly_transparent_edge_labels_get_a_cutout_on_an_opaque_canvas() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br#"{"schema_version":2,"extends":"studio",
                 "form":{"edge_label_bg":"transparent","elevation":"flat"}}"#,
            None,
        )
        .unwrap();
    let canvas = Color::rgb(255, 255, 255);
    for keyword in [
        "flowchart",
        #[cfg(feature = "diagram-swimlane")]
        "swimlane-beta",
    ] {
        for html_labels in [false, true] {
            let source = format!(
                "---\nconfig:\n  layout: shoal\n  htmlLabels: {html_labels}\n---\n\
                 {keyword} LR\nA e@-->|Caption| B\n"
            );
            for resvg in [false, true] {
                let svg = render_with_theme(
                    &source,
                    canvas,
                    resvg,
                    Some(NativeTheme::new(theme.clone())),
                );
                assert!(!svg.contains("merman-edge-label-background"));
                assert_label_cutout(&svg);
            }
            #[cfg(feature = "layout-dagre")]
            {
                let dagre = render_with_theme(
                    &source.replace("layout: shoal", "layout: dagre"),
                    canvas,
                    false,
                    Some(NativeTheme::new(theme.clone())),
                );
                assert!(!dagre.contains("-label-cutout"));
            }
        }
    }
}

fn assert_label_cutout(svg: &str) {
    let doc = Document::parse(svg).unwrap();
    let edge = doc
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    let clip_id = edge
        .attribute("clip-path")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|node| node.attribute("id") == Some(clip_id))
            .count(),
        1
    );
    let clip = doc
        .descendants()
        .find(|node| node.has_tag_name("clipPath") && node.attribute("id") == Some(clip_id))
        .unwrap()
        .first_element_child()
        .unwrap();
    assert_eq!(clip.attribute("clip-rule"), Some("evenodd"));
    let geometry = BezPath::from_svg(clip.attribute("d").unwrap()).unwrap();
    let center = doc
        .descendants()
        .find(|node| {
            (node.attribute("class") == Some("edgeLabel")
                || node.attribute("data-et") == Some("edge-label"))
                && node.attribute("transform").is_some()
        })
        .unwrap()
        .attribute("transform")
        .unwrap();
    let (x, y) = center
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    let center = Point::new(x.trim().parse().unwrap(), y.trim().parse().unwrap());
    assert_eq!(
        geometry.winding(center) % 2,
        0,
        "label center is in the cutout"
    );
    let shaft = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
    assert_eq!(
        geometry.winding(shaft.bounding_box().origin()) % 2,
        1,
        "shaft remains visible outside its label"
    );
}

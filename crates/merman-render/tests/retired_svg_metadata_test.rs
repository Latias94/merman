use merman_core::{Engine, ManagedSemanticJson, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{IshikawaDiagramLayout, TreeViewDiagramLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;
use std::collections::BTreeSet;

fn render(source: &str, config: MermaidConfig) -> (ManagedSemanticJson, String) {
    let parsed = Engine::new()
        .with_site_config(config)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse metadata regression fixture")
        .expect("detect metadata regression fixture");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_json().unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("retired-metadata".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render metadata regression fixture");
    (layout, rendered.svg().to_owned())
}

fn assert_live_ids_and_references(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid SVG");
    let mut ids = BTreeSet::new();
    for id in document
        .descendants()
        .filter_map(|node| node.attribute("id"))
    {
        assert!(ids.insert(id), "duplicate live SVG id: {id}");
    }
    let references = regex::Regex::new(r"url\(#([^\)]+)\)").unwrap();
    for reference in references.captures_iter(svg) {
        assert!(
            ids.contains(&reference[1]),
            "unresolved local SVG reference: {}",
            &reference[1]
        );
    }
    assert!(
        ids.len() > 1,
        "fixture must exercise live document identities"
    );
}

#[test]
fn ishikawa_omits_retired_text_bbox_metadata_but_keeps_layout_and_text_geometry() {
    let source = "ishikawa-beta\n Root cause\n  Process\n   Slow step\n   Missing spec\n  People\n   Missing owner\n";
    for look in ["classic", "handDrawn"] {
        let (projection, svg) = render(
            source,
            MermaidConfig::from_value(json!({
                "look": look, "handDrawnSeed": 7
            })),
        );
        assert!(
            !svg.contains("data-merman-text-bbox"),
            "retired bbox metadata must not be serialized for {look}"
        );
        let layout: IshikawaDiagramLayout =
            serde_json::from_value(projection["layout"]["IshikawaDiagram"].clone()).unwrap();
        let head = layout.head.as_ref().expect("head layout remains available");
        let mut labels = vec![&head.label];
        for pair in &layout.pairs {
            for branch in std::iter::once(&pair.upper).chain(pair.lower.iter()) {
                labels.push(&branch.label_group.label);
                labels.extend(branch.sub_groups.iter().map(|group| &group.label));
            }
        }
        assert_eq!(labels.len(), 6);
        for label in &labels {
            assert!(label.bbox.max_x > label.bbox.min_x);
            assert!(label.bbox.max_y > label.bbox.min_y);
        }
        let document = roxmltree::Document::parse(&svg).unwrap();
        let texts = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        assert_eq!(texts.len(), labels.len());
        for (text, label) in texts.iter().zip(&labels) {
            assert_eq!(text.attribute("class"), Some(label.class_name.as_str()));
            assert_eq!(text.attribute("text-anchor"), Some(label.anchor.as_str()));
            let content = text
                .descendants()
                .filter(roxmltree::Node::is_text)
                .filter_map(|node| node.text())
                .collect::<String>();
            assert_eq!(content, label.lines.join(""));
            let first_line_offset =
                -(label.lines.len().saturating_sub(1) as f64 * label.line_height) / 2.0;
            let (expected_x, expected_y) = if label.class_name == "ishikawa-head-label" {
                let transform = text.attribute("transform").unwrap();
                let translation = transform
                    .strip_prefix("translate(")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap()
                    .split(',')
                    .map(|value| value.trim().parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(translation.len(), 2);
                assert!((translation[0] - (label.x - head.x)).abs() < 1e-9);
                assert!((translation[1] - (label.y - head.y)).abs() < 1e-9);
                (0.0, first_line_offset)
            } else {
                (label.x, label.y + first_line_offset)
            };
            for (attribute, expected) in [("x", expected_x), ("y", expected_y)] {
                let actual = text.attribute(attribute).unwrap().parse::<f64>().unwrap();
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "{attribute}: {actual} vs {expected}"
                );
            }
        }
        let viewbox = document.root_element().attribute("viewBox").unwrap();
        let bounds = viewbox
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            bounds,
            vec![
                layout.viewbox_x,
                layout.viewbox_y,
                layout.total_width,
                layout.total_height
            ]
        );
        if look == "classic" {
            assert_live_ids_and_references(&svg);
        }
    }
}

#[test]
fn tree_view_omits_retired_edge_ids_but_keeps_line_geometry_and_paint() {
    let (projection, svg) = render(
        "treeView-beta\nRoot/\n    Child\n        Leaf\n",
        MermaidConfig::from_value(json!({
            "themeVariables": {"treeView": {"lineColor": "#123456"}}
        })),
    );
    assert!(!svg.contains("-merman-fill-stroke-paint"));
    assert!(!svg.contains("-merman-native-paint"));
    let layout: TreeViewDiagramLayout =
        serde_json::from_value(projection["layout"]["TreeViewDiagram"].clone()).unwrap();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let lines = document
        .descendants()
        .filter(|node| node.has_tag_name("line"))
        .collect::<Vec<_>>();
    assert!(!lines.is_empty());
    assert_eq!(lines.len(), layout.lines.len());
    for (line, expected) in lines.iter().zip(&layout.lines) {
        assert_eq!(line.attribute("id"), None);
        assert_eq!(line.attribute("class"), Some("treeView-node-line"));
        for (attribute, value) in [
            ("x1", expected.x1),
            ("y1", expected.y1),
            ("x2", expected.x2),
            ("y2", expected.y2),
            ("stroke-width", expected.stroke_width),
        ] {
            let actual = line.attribute(attribute).unwrap().parse::<f64>().unwrap();
            assert!(
                (actual - value).abs() < 1e-9,
                "{attribute}: {actual} vs {value}"
            );
        }
    }
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".treeView-node-line { stroke: #123456; }"));
    assert!(css.contains("font-family:"));
}

#[test]
fn hand_drawn_clusters_omit_retired_path_ids_but_keep_authored_paint_and_live_ids() {
    let source = "flowchart TB\nsubgraph Lane[Authored lane]\n A[Alpha] --> B[Beta]\nend\nstyle Lane fill:#123456,stroke:#654321\n";
    for layout in ["dagre", "swimlane"] {
        let (_, svg) = render(
            source,
            MermaidConfig::from_value(json!({
                "layout": layout, "look": "handDrawn", "handDrawnSeed": 7
            })),
        );
        assert!(!svg.contains("-merman-fill-paint"), "{layout}");
        assert_live_ids_and_references(&svg);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let cluster = document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some("Lane")
                    && node.attribute("data-et") == Some("cluster")
            })
            .expect("live semantic cluster identity");
        assert!(cluster.attribute("id").is_some());
        assert_eq!(cluster.attribute("data-look"), Some("handDrawn"));
        let paths = cluster
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|path| {
            path.attribute("style")
                .is_some_and(|style| style.contains("stroke:#123456"))
        }));
        assert!(paths.iter().any(|path| {
            path.attribute("style")
                .is_some_and(|style| style.contains("stroke:#654321"))
        }));
        assert!(paths.iter().all(|path| path.attribute("id").is_none()));
    }
}

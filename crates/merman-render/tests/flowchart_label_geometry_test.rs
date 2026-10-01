use futures::executor::block_on;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::model::FlowchartLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};

const ASSETS: &[&str] = &[
    "icon: \"fa:bell\"",
    "icon: \"fa:bell\", form: \"circle\"",
    "icon: \"fa:bell\", form: \"square\"",
    "icon: \"fa:bell\", form: \"rounded\"",
    "img: \"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0OCIgaGVpZ2h0PSI0OCIvPg==\", w: 48, h: 48, constraint: \"on\"",
];

const LAYOUTS: &[&str] = if cfg!(feature = "layout-elk") {
    &["dagre", "elk"]
} else {
    &["dagre"]
};

fn render(source: &str, layout: &str) -> (FlowchartLayout, String) {
    let parsed = block_on(
        Engine::new()
            .with_site_config(MermaidConfig::from_value(
                serde_json::json!({"layout": layout}),
            ))
            .parse_diagram_for_render_model(source, ParseOptions::default()),
    )
    .expect("parse diagram")
    .expect("detect flowchart");
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare flowchart");
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .expect("flowchart layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG");
    (layout, svg.svg().to_owned())
}

fn source_node<'a>(document: &'a roxmltree::Document<'_>) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("flowchart-A-0"))
        })
        .expect("node A")
}

fn dimension(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .expect("dimension")
        .parse()
        .expect("numeric dimension")
}

fn assert_close(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "{context}: actual {actual}, expected {expected}"
    );
}

#[test]
fn ordinary_html_labels_retain_measured_visual_boxes() {
    for layout_mode in LAYOUTS {
        for shape in ["rect", "delay", "doc", "folder"] {
            for (label, expected_height) in [
                ("<br/><br/>", 42.0),
                ("<i class='fa fa-car'></i>", 21.0),
                ("", 0.0),
            ] {
                let source = format!(
                    "flowchart LR\nA@{{ shape: {shape}, label: \"{label}\" }} --> B[Rect]\n"
                );
                let (_, svg) = render(&source, layout_mode);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let label_box = source_node(&document)
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .expect("HTML label box");
                assert_close(dimension(label_box, "height"), expected_height, &source);
            }
        }
    }
}

// The asset renderer emits a rectangular path for its complete layout box, separate from the
// icon/image frame. Parse that exact path instead of estimating a curved frame's bounds.
fn asset_box_size(group: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let path = group
        .children()
        .filter(|child| child.has_tag_name("g"))
        .flat_map(|child| child.children())
        .rfind(|child| {
            child.has_tag_name("path")
                && child.attribute("stroke") == Some("none")
                && matches!(child.attribute("fill"), Some("none" | "transparent"))
        })
        .expect("outer asset box");
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for segment in svgtypes::PathParser::from(path.attribute("d").unwrap()) {
        match segment.expect("valid outer box path") {
            svgtypes::PathSegment::MoveTo { abs: true, x, y }
            | svgtypes::PathSegment::LineTo { abs: true, x, y } => {
                xs.push(x);
                ys.push(y);
            }
            other => panic!("outer box must contain only absolute line segments: {other:?}"),
        }
    }
    let extent = |values: &[f64]| {
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - values.iter().copied().fold(f64::INFINITY, f64::min)
    };
    (extent(&xs), extent(&ys))
}

#[test]
fn asset_labels_use_layout_metrics_and_authored_font_styles() {
    for layout_mode in LAYOUTS {
        for asset in ASSETS {
            for style in [
                "classDef big font-size:32px\nclass A big",
                "style A font-size:32px",
            ] {
                for (label, expected_height) in [
                    ("Client", 48.0),
                    ("<i class='fa fa-car'></i>", 48.0),
                    ("<br/><br/>", 96.0),
                    ("", 0.0),
                ] {
                    for pos in ["t", "b"] {
                        let source = format!(
                            "flowchart LR\nA@{{ {asset}, label: \"{label}\", pos: \"{pos}\" }} --> B[Rect]\n{style}\n"
                        );
                        let (layout, svg) = render(&source, layout_mode);
                        let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
                        let document = roxmltree::Document::parse(&svg).unwrap();
                        let group = source_node(&document);
                        let label_box = group
                            .descendants()
                            .find(|node| node.has_tag_name("foreignObject"))
                            .unwrap();
                        let background_padding = if label.is_empty() { 0.0 } else { 4.0 };
                        assert_close(
                            dimension(label_box, "height"),
                            expected_height + background_padding,
                            &source,
                        );
                        let (width, height) = asset_box_size(group);
                        assert_close(width, node.width, &source);
                        assert_close(height, node.height, &source);
                        let span = label_box
                            .descendants()
                            .find(|node| node.has_tag_name("span"))
                            .unwrap();
                        assert!(
                            span.attribute("style")
                                .unwrap_or_default()
                                .contains("font-size:32px"),
                            "authored font must reach the HTML label: {source}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn asset_labels_wrap_at_the_measured_width() {
    for layout_mode in LAYOUTS {
        for asset in ASSETS {
            let source = format!(
                "---\nconfig:\n  flowchart:\n    wrappingWidth: 120\n---\nflowchart LR\nA@{{ {asset}, label: \"Client portal account\" }} --> B[Rect]\nstyle A font-size:32px\n"
            );
            let (layout, svg) = render(&source, layout_mode);
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let group = source_node(&document);
            let label_box = group
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert!(
                dimension(label_box, "height") > 52.0,
                "long label must occupy multiple lines: {source}"
            );
            let div = label_box
                .descendants()
                .find(|node| node.has_tag_name("div"))
                .unwrap();
            let style = div.attribute("style").unwrap();
            assert!(style.contains("display: table;"), "{source}: {style}");
            assert!(
                style.contains("white-space: break-spaces;"),
                "{source}: {style}"
            );
            assert!(style.contains("; width: 120px;"), "{source}: {style}");
            let (width, height) = asset_box_size(group);
            assert_close(width, node.width, &source);
            assert_close(height, node.height, &source);
        }
    }
}

#[test]
fn image_layout_and_paint_share_size_constraints() {
    for layout_mode in LAYOUTS {
        for wrapping_width in [120.0, 240.0] {
            for (attributes, asset_width, asset_height, explicit_height) in [
                ("", 60.0, 60.0, false),
                (", w: 90, h: 60", 90.0, 60.0, true),
            ] {
                for constraint in ["on", "off"] {
                    for label in ["Client", "<br/><br/>", ""] {
                        let source = format!(
                            "---\nconfig:\n  flowchart:\n    wrappingWidth: {wrapping_width}\n---\nflowchart LR\nA@{{ img: \"data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0OCIgaGVpZ2h0PSI0OCIvPg==\", label: \"{label}\", constraint: \"{constraint}\"{attributes} }} --> B[Rect]\n"
                        );
                        let (layout, svg) = render(&source, layout_mode);
                        let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
                        let document = roxmltree::Document::parse(&svg).unwrap();
                        let group = source_node(&document);
                        let image = group
                            .descendants()
                            .find(|node| node.has_tag_name("image"))
                            .unwrap();
                        let expected_width =
                            if constraint == "on" && explicit_height || label.is_empty() {
                                asset_width
                            } else {
                                wrapping_width
                            };
                        let expected_height = if constraint == "on" {
                            expected_width / (asset_width / asset_height)
                        } else {
                            asset_height
                        };
                        assert_close(dimension(image, "width"), expected_width, &source);
                        assert_close(dimension(image, "height"), expected_height, &source);
                        let (width, height) = asset_box_size(group);
                        assert_close(width, node.width, &source);
                        assert_close(height, node.height, &source);
                    }
                }
            }
        }
    }
}

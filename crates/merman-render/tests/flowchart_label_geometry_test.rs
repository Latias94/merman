use base64::Engine as _;
use futures::executor::block_on;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::model::{FlowchartLayout, LayoutPoint};
use merman_render::svg::{
    SvgDebugOptions, SvgRenderOptions, foreign_object_label_fallback_svg_text,
};
use merman_render::text::DeterministicTextMeasurer;
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

// The embedded PNG is 120 × 60 pixels.
const WIDE_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAHgAAAA8CAIAAAAiz+n/AAAAn0lEQVR4nO3QMQ0AIADAMEAF8hCJQFSwqzWwZHOfO/hvBQ2M7hgdMTpidMToiNERoyNGR4yOGB0xOmJ0xOiI0RGjI0ZHjI4YHTE6YnTE6IjREaMjRkeMjhgdMTpidMToiNERoyNGR4yOGB0xOmJ0xOiI0RGjI0ZHjI4YHTE6YnTE6IjREaMjRkeMjhgdMTpidMToiNERoyNGR4yOGD0aD1QBAaT/CgOSAAAAAElFTkSuQmCC";

#[test]
fn html_label_images_use_authored_height_or_intrinsic_aspect_ratio() {
    let config = "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 400\n---\nflowchart TB\n";
    for mode in LAYOUTS {
        let (_, baseline) = render(&format!("{config}A[Alpha Beta]\n"), mode);
        let (text_w, text_h) = first_foreign_object_size(&baseline);
        for (attrs, text, expected) in [
            (
                " width='30' height='15'",
                " Alpha Beta",
                (text_w, text_h + 15.0),
            ),
            (" width='30'", " Alpha Beta", (120.0, text_h + 60.0)),
            ("", "", (80.0, 40.0)),
            (" height='15'", "", (80.0, 15.0)),
        ] {
            let source = format!("{config}A[\"<img src='{WIDE_PNG}'{attrs}/>{text}\"]\n");
            let (layout, svg) = render(&source, mode);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let img = doc
                .descendants()
                .find(|node| node.has_tag_name("img"))
                .unwrap();
            assert_eq!(img.attribute("src"), Some(WIDE_PNG));
            assert_eq!(
                img.attribute("width"),
                attrs.contains("width=").then_some("30")
            );
            assert_eq!(
                img.attribute("height"),
                attrs.contains("height=").then_some("15")
            );
            let (width, height) = first_foreign_object_size(&svg);
            assert!(
                (width - expected.0).abs() <= 1.0 / 64.0,
                "{mode} {attrs}: {width}"
            );
            assert_eq!(height, expected.1, "{mode} {attrs}");
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            assert_eq!(node.height, height + 24.0, "{mode} {attrs}");
        }
        let style = "style A letter-spacing:12px\n";
        let (_, text) = render(&format!("{config}A[Alpha Beta]\n{style}"), mode);
        let (_, mixed) = render(
            &format!("{config}A[\"<img src='{WIDE_PNG}' height='15'/> Alpha Beta\"]\n{style}"),
            mode,
        );
        let (text_w, text_h) = first_foreign_object_size(&text);
        assert_eq!(
            first_foreign_object_size(&mixed),
            (text_w, text_h + 15.0),
            "{mode}"
        );
        for width in [60, 180, 480] {
            let (_, text) = render(
                &format!("{config}A[\"<div style='width:{width}px'>Alpha Beta</div>\"]\n"),
                mode,
            );
            let (_, mixed) = render(
                &format!(
                    "{config}A[\"<div style='width:{width}px'><img src='{WIDE_PNG}'/>Alpha Beta</div>\"]\n"
                ),
                mode,
            );
            let (text_width, text_height) = first_foreign_object_size(&text);
            assert_eq!(
                first_foreign_object_size(&mixed),
                (text_width, text_height + f64::from(width) / 2.0),
                "{mode} {width}"
            );
        }
        let (_, unavailable) = render(
            &format!(
                "{config}A[\"<div style='width:180px'><img src='missing.png'/>Alpha Beta</div>\"]\n"
            ),
            mode,
        );
        assert_eq!(first_foreign_object_size(&unavailable), (180.0, 221.0));
    }
}

fn first_foreign_object_size(svg: &str) -> (f64, f64) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let label = doc
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    let number = |name| label.attribute(name).unwrap().parse().unwrap();
    (number("width"), number("height"))
}

fn render(source: &str, layout: &str) -> (FlowchartLayout, String) {
    render_family(source, layout, "FlowchartV2")
}

fn render_family<T: serde::de::DeserializeOwned>(
    source: &str,
    layout: &str,
    family: &str,
) -> (T, String) {
    let parsed = block_on(
        MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"layout": layout}),
                ),
            )
            .parse_diagram_for_render_model(source, ParseOptions::default()),
    )
    .expect("parse diagram")
    .expect("detect flowchart")
    .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare flowchart");
    let layout = serde_json::from_value(artifact.layout_json().unwrap()["layout"][family].clone())
        .expect("diagram layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG");
    (layout, svg.svg().to_owned())
}

#[test]
fn nonfinite_asset_dimensions_fail_before_layout_for_each_shared_consumer() {
    for layout_mode in LAYOUTS {
        for keyword in std::iter::once("flowchart")
            .chain(cfg!(feature = "diagram-swimlane").then_some("swimlane-beta"))
        {
            for asset in ASSETS {
                for property in ["w", "h"] {
                    for value in [r#""bad""#, r#""Infinity""#, "[1, 2]"] {
                        let source = format!(
                            "{keyword} LR\nA@{{ {asset} }}\nA@{{ {property}: {value} }}\nA --> B\n"
                        );
                        let parsed = MermaidEngine::new()
                            .with_site_config(
                                merman_core::presentation::SourcePresentationConfig::from_value(
                                    serde_json::json!({"layout": layout_mode}),
                                ),
                            )
                            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                            .expect("invalid dimensions remain valid source")
                            .expect("detect diagram")
                            .into_presentation();
                        let error = family::prepare(
                            &parsed,
                            &LayoutOptions::default(),
                            RenderEnvironment::deterministic().begin_session().unwrap(),
                        )
                        .err()
                        .expect("non-finite asset geometry cannot reach a layout provider");
                        let merman_render::Error::InvalidModel { message } = error else {
                            panic!("unexpected error {error}: {source}");
                        };
                        let dimension = if property == "w" { "width" } else { "height" };
                        assert_eq!(
                            message,
                            format!("Flowchart node A has non-finite asset {dimension}"),
                            "{source}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unused_or_replaced_nonfinite_asset_dimensions_do_not_block_ordinary_geometry() {
    for layout_mode in LAYOUTS {
        for source in [
            "flowchart LR\nA@{ w: \"bad\", h: \"Infinity\" } --> B\n",
            "flowchart LR\nA@{ icon: \"fa:bell\", w: \"bad\", h: \"Infinity\" }\nA@{ w: 48, h: 48 } --> B\n",
        ] {
            let (_, svg) = render(source, layout_mode);
            assert!(!svg.contains("NaN"), "{source}");
            assert!(!svg.contains("Infinity"), "{source}");
            roxmltree::Document::parse(&svg).expect("well-formed SVG");
        }
    }
}

#[test]
fn hidden_or_group_replaced_assets_do_not_validate_unused_dimensions() {
    for layout_mode in LAYOUTS {
        let source = "flowchart LR\nsubgraph G\nA@{ icon: \"fa:bell\", w: \"bad\" }\nend\nG@{ view: collapsed }\n";
        let (layout, svg) = render(source, layout_mode);
        assert!(!layout.nodes.iter().any(|node| node.id == "A"));
        assert!(!svg.contains("NaN"));
        roxmltree::Document::parse(&svg).unwrap();
        for keyword in std::iter::once("flowchart")
            .chain(cfg!(feature = "diagram-swimlane").then_some("swimlane-beta"))
        {
            let source = format!(
                "{keyword} LR\nG@{{ icon: \"fa:bell\", w: \"bad\" }}\nsubgraph G\nA\nend\n"
            );
            let parsed = MermaidEngine::new()
                .with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"layout": layout_mode}),
                    ),
                )
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(!svg.svg().contains("NaN"), "{source}");
            assert!(!svg.svg().contains("Infinity"), "{source}");
            roxmltree::Document::parse(svg.svg()).unwrap();
        }
    }
}

fn source_node<'a>(document: &'a roxmltree::Document<'_>) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("A")
                && node.attribute("data-et") == Some("node")
        })
        .unwrap_or_else(|| {
            panic!(
                "node A not found; group IDs: {:?}",
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("g"))
                    .filter_map(|node| node.attribute("id"))
                    .collect::<Vec<_>>()
            )
        })
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
fn dagre_routes_external_edges_against_the_styled_recursive_subgraph_footprint() {
    for (connection, expected_route_y) in [("C --> G", 103.0), ("G --> C", 228.0)] {
        let source = format!(
            "---\nconfig:\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nsubgraph G[Group]\nA[Short] --> B[End]\nend\n{connection}\nclassDef large width:380px,height:220px\nclass G large\n"
        );
        let (layout, svg) = render(&source, "dagre");
        let footprint = layout.nodes.iter().find(|node| node.id == "G").unwrap();
        assert_close(footprint.width, 380.0, connection);
        assert_close(footprint.height, 220.0, connection);
        let cluster = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "G")
            .unwrap();
        assert!(cluster.width < footprint.width);
        assert!(cluster.height < footprint.height);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let path = document
            .descendants()
            .find(|element| {
                element.has_tag_name("path")
                    && element.attribute("data-points").is_some()
                    && element
                        .ancestors()
                        .filter(|parent| parent.attribute("class") == Some("root"))
                        .count()
                        == 1
            })
            .expect("external edge");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
        let endpoint = if connection.starts_with('C') {
            points.last()
        } else {
            points.first()
        }
        .unwrap();
        assert_close(endpoint.x, 198.0, connection);
        assert_close(endpoint.y, expected_route_y, connection);
        let rendered_path = path.attribute("d").unwrap();
        if connection.starts_with('C') {
            assert!(rendered_path.ends_with("198,99"), "{rendered_path}");
        } else {
            assert!(rendered_path.starts_with("M198,228"), "{rendered_path}");
        }
    }
}

#[test]
fn image_nodes_use_intrinsic_dimensions_for_layout_and_svg() {
    for backend in LAYOUTS {
        for (properties, expected) in [
            ("", (120.0, 60.0)),
            (", w: 90", (90.0, 60.0)),
            (", w: 90, constraint: on", (90.0, 45.0)),
            (", h: 40, constraint: off", (120.0, 40.0)),
            (", h: 40, constraint: on", (80.0, 40.0)),
            (", w: 90, h: 40, constraint: off", (90.0, 40.0)),
            (", w: 90, h: 40, constraint: on", (80.0, 40.0)),
            (r#", w: 90, h: "0", constraint: on"#, (90.0, 45.0)),
        ] {
            let source = format!("flowchart LR\nA@{{ img: \"{WIDE_PNG}\"{properties} }} --> B\n");
            let (layout, svg) = render(&source, backend);
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let context = format!("{backend}{properties}");
            assert_close(node.width, expected.0, &context);
            assert_close(node.height, expected.1, &context);
            assert_close(
                layout.edges[0].points[0].x,
                node.x + expected.0 / 2.0,
                &context,
            );

            let document = roxmltree::Document::parse(&svg).unwrap();
            let image = source_node(&document)
                .children()
                .find(|node| node.has_tag_name("image"))
                .unwrap();
            assert_eq!(image.attribute("href"), Some(WIDE_PNG));
            assert_close(dimension(image, "width"), expected.0, &context);
            assert_close(dimension(image, "height"), expected.1, &context);
        }
    }
}

#[test]
fn cluster_css_size_uses_the_frame_origin_and_painted_viewport() {
    let source = "---\nconfig:\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nsubgraph G[Group]\nA[Short] --> B[End]\nend\nC --> A\n";
    for backend in LAYOUTS {
        let (natural, natural_svg) = render(source, backend);
        let natural_view = view_box(&natural_svg);
        for style in [
            "style G width:260px,height:180px,stroke:none\n",
            "classDef sized width:260px,height:180px,stroke:none\nclass G sized\n",
            "classDef sized width:260px !important,height:180px,stroke:none\nstyle G width:200px\nclass G sized\n",
        ] {
            let (layout, svg) = render(&format!("{source}{style}"), backend);
            let frame = &layout.clusters[0];
            assert_eq!(frame.width, natural.clusters[0].width);
            assert_eq!(frame.height, natural.clusters[0].height);

            let document = roxmltree::Document::parse(&svg).unwrap();
            let rect = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.parent().is_some_and(|group| {
                            group.attribute("data-id") == Some("G")
                                && group.attribute("data-et") == Some("cluster")
                        })
                })
                .unwrap();
            assert_close(dimension(rect, "x"), frame.x - frame.width / 2.0, backend);
            assert_close(dimension(rect, "y"), frame.y - frame.height / 2.0, backend);
            assert_close(dimension(rect, "width"), 260.0, backend);
            assert_close(dimension(rect, "height"), 180.0, backend);

            let view = view_box(&svg);
            assert_close(view[2], 276.0, backend);
            assert_close(view[3], natural_view[3] - (frame.height - 180.0), backend);
        }
    }
}

fn view_box(svg: &str) -> Vec<f64> {
    let document = roxmltree::Document::parse(svg).unwrap();
    document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect()
}

#[test]
fn image_node_label_changes_the_box_and_respects_the_width_constraint() {
    for backend in LAYOUTS {
        for (constraint, image_width, image_height) in [("off", 150.0, 40.0), ("on", 80.0, 40.0)] {
            let source = format!(
                "---\nconfig:\n  flowchart:\n    wrappingWidth: 150\n---\nflowchart TB\nA@{{ img: \"{WIDE_PNG}\", label: \"Logo\", h: 40, constraint: {constraint}, pos: t }} --> B\n"
            );
            let (layout, svg) = render(&source, backend);
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let group = source_node(&document);
            let image = group
                .children()
                .find(|node| node.has_tag_name("image"))
                .unwrap();
            let label = group
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            let context = format!("{backend}, constraint: {constraint}");
            assert_close(dimension(image, "width"), image_width, &context);
            assert_close(dimension(image, "height"), image_height, &context);
            assert_close(
                node.width,
                image_width.max(dimension(label, "width")),
                &context,
            );
            assert_close(
                node.height,
                image_height + dimension(label, "height") + 8.0,
                &context,
            );
        }
    }
}

#[test]
fn labeled_image_edges_clip_to_the_image_and_label_outline() {
    for backend in LAYOUTS {
        for pos in ["t", "b"] {
            let source = format!(
                "flowchart LR\nA@{{ img: \"{WIDE_PNG}\", h: 30, constraint: on, label: \"Logo\", pos: {pos} }} --> B\n"
            );
            let (layout, svg) = render(&source, backend);
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = source_node(&document)
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            let label_half_width = dimension(label, "width") / 2.0;
            let label_height = dimension(label, "height") + 8.0;
            let encoded = document
                .descendants()
                .find_map(|node| node.attribute("data-points"))
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(
                &base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .unwrap(),
            )
            .unwrap();
            assert_close(points[0].y, node.y, backend);
            let end = if pos == "t" {
                label_half_width
            } else {
                // The lower right edge of the source polygon tapers across the label band.
                label_half_width * (1.0 - (node.height / 2.0 - 30.0) / (2.0 * label_height))
            };
            assert!(
                (points[0].x - node.x - end).abs() < 1e-4,
                "{backend}, pos {pos}: {points:?}"
            );
        }
    }
}

#[test]
fn unresolved_image_sources_keep_the_declared_size_fallback() {
    for source in [
        "https://example.com/image.png",
        "data:image/png;base64,eA==",
    ] {
        let (layout, svg) = render(
            &format!("flowchart TB\nA@{{ img: \"{source}\", h: 40, constraint: on }}\n"),
            "dagre",
        );
        let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!((node.width, node.height), (40.0, 40.0), "{source}");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let image = source_node(&document)
            .children()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        assert_eq!(image.attribute("href"), Some(source));
        assert_eq!(
            (dimension(image, "width"), dimension(image, "height")),
            (40.0, 40.0)
        );
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_image_nodes_use_the_same_intrinsic_dimensions() {
    let (layout, svg): (merman_render::model::SwimlaneLayout, _) = render_family(
        &format!(
            "swimlane-beta LR\nsubgraph S[Lane]\nA@{{ img: \"{WIDE_PNG}\", h: 40, constraint: on }} --> B\nend\n"
        ),
        "swimlane",
        "SwimlaneDiagram",
    );
    let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
    assert_eq!((node.width, node.height), (80.0, 40.0));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let image = source_node(&document)
        .children()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert_eq!(
        (dimension(image, "width"), dimension(image, "height")),
        (80.0, 40.0)
    );
}

#[test]
fn rectangle_css_size_controls_layout_paint_and_viewport() {
    let source = "---\nconfig:\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nA[Short] --> B[End]\n";
    for backend in LAYOUTS {
        let (natural, _) = render(source, backend);
        let natural_node = &natural.nodes[0];
        for style in [
            "style A width:220px,height:90px\n",
            "classDef sized width:220px,height:90px\nclass A sized\n",
            "classDef sized width:180px,height:80px\nclass A sized\nstyle A width:220px,height:90px\n",
            "style A width:220,height:90\n",
            "style A width:180px,WIDTH:220px,width:200px,height:90px\n",
            "classDef sized width:220px !important,height:90px !important\nclass A sized\nstyle A WIDTH:180px,HEIGHT:80px\n",
            "classDef sized WIDTH:180px,HEIGHT:80px\nclass A sized\nstyle A width:220px !important,height:90px !important\n",
        ] {
            let context = format!("{backend}: {style}");
            let (layout, svg) = render(&format!("{source}{style}"), backend);
            let node = &layout.nodes[0];
            assert_close(node.width, 220.0, &context);
            assert_close(node.height, 90.0, &context);
            let start = &layout.edges[0].points[0];
            assert_close(start.y, node.y + 45.0, &context);
            assert_ne!(start.y, natural.edges[0].points[0].y, "{context}");

            let document = roxmltree::Document::parse(&svg).unwrap();
            let rect = source_node(&document)
                .children()
                .find(|node| node.has_tag_name("rect"))
                .expect("node rectangle");
            assert_close(dimension(rect, "x"), -natural_node.width / 2.0, &context);
            assert_close(dimension(rect, "y"), -natural_node.height / 2.0, &context);
            assert_close(dimension(rect, "width"), 220.0, &context);
            assert_close(dimension(rect, "height"), 90.0, &context);
            let view_box: Vec<f64> = document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_ascii_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            // The left edge is the painted rectangle's original offset from A's new center.
            assert_close(
                view_box[0],
                node.x - natural_node.width / 2.0 - 8.0,
                &context,
            );
            assert_close(view_box[2], 220.0 + 16.0, &context);
        }
    }
}

#[test]
fn rectangle_css_size_includes_the_centered_label_when_the_shape_shrinks() {
    let source = "---\nconfig:\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nA[Short] --> B[End]\n";
    for backend in LAYOUTS {
        let (natural, _) = render(source, backend);
        let natural_node = &natural.nodes[0];
        for (source_width, source_height, width, height, lost_width, lost_height) in [
            ("10px", "10px", 10.0, 10.0, 16.0, 12.0),
            ("0", "90px", 0.0, 90.0, 32.0, 24.0),
            ("auto", "90px", 0.0, 90.0, 32.0, 24.0),
        ] {
            let (layout, svg) = render(
                &format!("{source}style A width:{source_width},height:{source_height}\n"),
                backend,
            );
            let node = &layout.nodes[0];
            assert_close(node.width, natural_node.width - lost_width, backend);
            assert_close(node.height, natural_node.height - lost_height, backend);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let rect = source_node(&doc)
                .children()
                .find(|node| node.has_tag_name("rect"))
                .expect("node rectangle");
            assert_close(dimension(rect, "width"), width, backend);
            assert_close(dimension(rect, "height"), height, backend);
        }
    }
}

#[test]
fn rounded_rectangle_css_width_keeps_the_natural_height_and_position() {
    let source = "---\nconfig:\n  look: classic\n  htmlLabels: false\n---\nflowchart LR\nA(Short) --> B(End)\n";
    for backend in LAYOUTS {
        let (base, _) = render(source, backend);
        let (layout, svg) = render(&format!("{source}style A width:2in\n"), backend);
        let node = &layout.nodes[0];
        assert_close(node.width, 192.0, backend);
        assert_close(node.height, base.nodes[0].height, backend);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rect = source_node(&document)
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert!(rect.attribute("rx").is_some());
        assert_close(dimension(rect, "x"), -base.nodes[0].width / 2.0, backend);
        assert_close(dimension(rect, "width"), 192.0, backend);
        assert_close(dimension(rect, "height"), base.nodes[0].height, backend);

        let encoded_points = document
            .descendants()
            .find_map(|element| element.attribute("data-points"))
            .unwrap();
        let points = base64::engine::general_purpose::STANDARD
            .decode(encoded_points)
            .unwrap();
        let points: Vec<LayoutPoint> = serde_json::from_slice(&points).unwrap();
        let route = &layout.edges[0].points;
        for (actual, expected) in [
            (&points[0], &route[0]),
            (points.last().unwrap(), route.last().unwrap()),
        ] {
            assert_close(actual.x, expected.x, backend);
            assert_close(actual.y, expected.y, backend);
        }
    }
}

#[test]
fn zero_area_rectangle_with_an_empty_label_routes_from_its_center() {
    let source = "---\nconfig:\n  htmlLabels: true\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nA[\"<div style='width:0;height:0'></div>\"] --> B[End]\nstyle A width:0,height:0\n";
    for backend in LAYOUTS {
        let (layout, _) = render(source, backend);
        let node = &layout.nodes[0];
        assert_eq!((node.width, node.height), (0.0, 0.0));
        let start = &layout.edges[0].points[0];
        assert_close(start.x, node.x, backend);
        assert_close(start.y, node.y, backend);
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_rectangle_css_size_reaches_layout_and_paint() {
    let source = "---\nconfig:\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nswimlane-beta TB\nsubgraph West\nA[Short]\nend\nsubgraph East\nB[End]\nend\nA --> B\n";
    let (layout, svg): (merman_render::model::SwimlaneLayout, _) = render_family(
        &format!("{source}style A width:220px,height:90px\n"),
        "swimlane",
        "SwimlaneDiagram",
    );
    let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
    assert_close(node.width, 220.0, "Swimlane node");
    assert_close(node.height, 90.0, "Swimlane node");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let rect = source_node(&document)
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert_close(dimension(rect, "width"), 220.0, "Swimlane shape");
    assert_close(dimension(rect, "height"), 90.0, "Swimlane shape");
}

#[test]
fn ordinary_html_labels_retain_measured_visual_boxes() {
    for layout_mode in LAYOUTS {
        for shape in ["rect", "delay", "doc", "folder"] {
            for (definition, label, expected_height, expected_text) in [
                ("A", "<br/><br/>", 42.0, ""),
                ("A", "<i class='fa fa-car'></i>", 21.0, ""),
                ("A", "", 21.0, "A"),
                ("A[Original]", "", 21.0, "Original"),
                (r#"A[" "]"#, "", 0.0, ""),
            ] {
                let source = format!(
                    "flowchart LR\n{definition}\nA@{{ shape: {shape}, label: \"{label}\" }} --> B[Rect]\n"
                );
                let (_, svg) = render(&source, layout_mode);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let label_box = source_node(&document)
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .expect("HTML label box");
                assert_close(dimension(label_box, "height"), expected_height, &source);
                let visible_text = label_box
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>();
                assert_eq!(visible_text.trim(), expected_text, "{source}");
            }
        }
    }
}

#[test]
fn flowchart_html_typography_sizes_labels_before_layout_and_native_projection() {
    let source = "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nA[\"Alpha Beta<br/>Gamma Delta\"] -->|Alpha Zeta| B[End]\n";
    for backend in LAYOUTS {
        let (base, _) = render(source, backend);
        let (styled, svg) = render(
            &format!(
                "{source}classDef tall line-height:3\nclass A tall\nlinkStyle 0 letter-spacing:12px\n"
            ),
            backend,
        );
        let node = styled.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_close(node.height, 108.0, backend);
        let edge_label = styled.edges[0].label.as_ref().unwrap();
        let plain_edge_label = base.edges[0].label.as_ref().unwrap();
        assert!((edge_label.width - plain_edge_label.width - 120.0).abs() < 1.0 / 64.0);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let html_label = source_node(&document)
            .descendants()
            .find(|element| element.has_tag_name("foreignObject"))
            .unwrap();
        assert_close(dimension(html_label, "height"), 84.0, backend);
        let fallback =
            foreign_object_label_fallback_svg_text(&svg, &DeterministicTextMeasurer::default());
        let document = roxmltree::Document::parse(&fallback).unwrap();
        let text = |label| {
            document
                .descendants()
                .find(|element| element.has_tag_name("text") && element.text() == Some(label))
                .unwrap_or_else(|| panic!("missing fallback text {label} in {backend}"))
        };
        let first = text("Alpha Beta");
        let second = text("Gamma Delta");
        assert_close(
            dimension(second, "y") - dimension(first, "y"),
            42.0,
            backend,
        );
        assert!(document.descendants().any(|element| {
            element.has_tag_name("text")
                && element.text() == Some("Alpha Zeta")
                && element
                    .attribute("style")
                    .is_some_and(|style| style.contains("letter-spacing: 12px"))
        }));
    }
}

#[test]
fn flowchart_html_word_spacing_and_transform_reach_layout_and_export() {
    let config = "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\n";
    for backend in LAYOUTS {
        let node_width = |label: &str| {
            let (layout, _) = render(&format!("{config}A[\"{label}\"]\n"), backend);
            layout.nodes[0].width
        };
        let (layout, svg) = render(
            &format!(
                "{config}A[\"Alpha Beta\"]\nstyle A word-spacing:20px,text-transform:uppercase\n"
            ),
            backend,
        );
        assert!((layout.nodes[0].width - node_width("ALPHA BETA") - 20.0).abs() < 1.0 / 64.0);
        let fallback =
            foreign_object_label_fallback_svg_text(&svg, &DeterministicTextMeasurer::default());
        let document = roxmltree::Document::parse(&fallback).unwrap();
        assert!(document.descendants().any(|element| {
            element.has_tag_name("text")
                && element.text() == Some("ALPHA BETA")
                && element
                    .attribute("style")
                    .is_some_and(|style| style.contains("word-spacing: 20px"))
        }));
    }
}

#[test]
fn flowchart_html_typography_affects_wrap_activation_and_block_line_boxes() {
    let config = "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 120\n---\nflowchart TB\n";
    for backend in LAYOUTS {
        for (source, height, mode) in [
            (
                "A[\"Alpha Beta\"]\nstyle A letter-spacing:12px",
                42.0,
                "break-spaces",
            ),
            (
                "A[\"Alpha Beta<br/>Gamma Delta\"]\nstyle A line-height:0.5",
                14.0,
                "nowrap",
            ),
            (
                "A[\"Alpha Beta<br/><br/>Gamma Delta\"]\nstyle A line-height:3",
                126.0,
                "nowrap",
            ),
            (
                "A[\"Alpha Beta<br/><span style='line-height:3'>Gamma Delta</span>\"]",
                63.0,
                "nowrap",
            ),
            (
                "A[\"<span style='font-size:32px'>Alpha Beta</span>\"]",
                96.0,
                "break-spaces",
            ),
            ("A[\"`one\n\ntwo`\"]\nstyle A line-height:3", 84.0, "nowrap"),
        ] {
            let (layout, svg) = render(&format!("{config}{source}\n"), backend);
            let context = format!("{backend}, {source}");
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            assert_close(node.height, height + 24.0, &context);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = source_node(&document)
                .descendants()
                .find(|element| element.has_tag_name("foreignObject"))
                .unwrap();
            assert_close(dimension(label, "height"), height, &context);
            assert!(
                label
                    .first_element_child()
                    .unwrap()
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("white-space: {mode};")),
                "{context}"
            );
        }
    }
}

#[test]
fn standalone_html_label_boxes_size_nodes_and_edge_endpoints() {
    let long_label = "one two three four five six seven eight nine ten eleven twelve";
    for layout_mode in LAYOUTS {
        for (style, minimum_width, label_width, short_label, long_label_box) in [
            (
                "width:160px;width:188px/**/;height:66px",
                120,
                188.0,
                (66.0, "break-spaces"),
                (66.0, "break-spaces"),
            ),
            (
                "width:188px",
                120,
                188.0,
                (21.0, "break-spaces"),
                (63.0, "break-spaces"),
            ),
            (
                "width:188px;height:12px",
                120,
                188.0,
                (12.0, "break-spaces"),
                (12.0, "break-spaces"),
            ),
            ("width:80px", 120, 120.0, (21.0, "nowrap"), (21.0, "nowrap")),
            (
                "width:120px",
                225,
                225.0,
                (21.0, "break-spaces"),
                (84.0, "break-spaces"),
            ),
            (
                "height:66px",
                120,
                120.0,
                (66.0, "nowrap"),
                (66.0, "break-spaces"),
            ),
            (
                "width:188.319PX!important;width:80px;height:0",
                120,
                188.3125,
                (0.0, "break-spaces"),
                (0.0, "break-spaces"),
            ),
        ] {
            let source = format!(
                "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    wrappingWidth: 120\n    minNodeWidth: {minimum_width}\n---\nflowchart TB\nA[\"<div style='{style}'>short</div>\"] --> B@{{ shape: rect, label: \"<div style='{style}'>{long_label}</div>\" }}\n"
            );
            let context = format!("{layout_mode}, {style}, min width {minimum_width}");
            let (layout, svg) = render(&source, layout_mode);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let node = |id| layout.nodes.iter().find(|node| node.id == id).unwrap();
            for (id, (label_height, white_space)) in [("A", short_label), ("B", long_label_box)] {
                let node = node(id);
                assert_close(node.width, label_width + 32.0, &context);
                assert_close(node.height, label_height + 24.0, &context);

                let group = document
                    .descendants()
                    .find(|group| {
                        group.attribute("data-id") == Some(id)
                            && group.attribute("data-et") == Some("node")
                    })
                    .unwrap();
                let rect = group
                    .children()
                    .find(|child| child.has_tag_name("rect"))
                    .unwrap();
                assert_close(dimension(rect, "width"), node.width, &context);
                assert_close(dimension(rect, "height"), node.height, &context);
                let label = group
                    .descendants()
                    .find(|child| child.has_tag_name("foreignObject"))
                    .unwrap();
                assert_close(dimension(label, "width"), label_width, &context);
                assert_close(dimension(label, "height"), label_height, &context);
                assert!(
                    label
                        .first_element_child()
                        .unwrap()
                        .attribute("style")
                        .unwrap()
                        .contains(&format!("white-space: {white_space};")),
                    "{context}"
                );
                assert_eq!(label.attribute("style"), None, "{context}");
            }

            let encoded_points = document
                .descendants()
                .find_map(|element| element.attribute("data-points"))
                .unwrap();
            let points = base64::engine::general_purpose::STANDARD
                .decode(encoded_points)
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&points).unwrap();
            assert_close(
                points.first().unwrap().y,
                node("A").y + node("A").height / 2.0,
                &context,
            );
            assert_close(
                points.last().unwrap().y,
                node("B").y - node("B").height / 2.0,
                &context,
            );
        }
    }
}

#[test]
fn standalone_html_label_wrap_uses_the_measured_outer_width_and_preserves_spaces() {
    let spaces = " ".repeat(42);
    for (wrapping_width, label_width, label_height, white_space) in [
        (120.0, 188.0, 42.0, "break-spaces"),
        (120.01, 120.0, 21.0, "nowrap"),
    ] {
        let source = format!(
            "---\nconfig:\n  look: neo\n  htmlLabels: true\n  flowchart:\n    wrappingWidth: {wrapping_width}\n---\nflowchart TB\nA@{{ shape: rect, label: \"<div style='width:188px'>short{spaces}short</div>\" }}\n"
        );
        let (layout, svg) = render(&source, "dagre");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = source_node(&document)
            .descendants()
            .find(|element| element.has_tag_name("foreignObject"))
            .unwrap();
        assert_close(layout.nodes[0].width, label_width + 32.0, &source);
        assert_close(layout.nodes[0].height, label_height + 24.0, &source);
        assert_close(dimension(label, "width"), label_width, &source);
        assert_close(dimension(label, "height"), label_height, &source);
        assert!(
            label
                .first_element_child()
                .unwrap()
                .attribute("style")
                .unwrap()
                .contains(&format!("white-space: {white_space};")),
            "{source}"
        );
    }
}

#[test]
fn standalone_html_label_fallback_uses_the_inner_box_for_text() {
    let source = "flowchart TB\nA[\"<div style='width:80px;height:66px'>short</div>\"]\n";
    let (_, svg) = render(source, "dagre");
    let fallback =
        foreign_object_label_fallback_svg_text(&svg, &DeterministicTextMeasurer::default());
    let document = roxmltree::Document::parse(&fallback).unwrap();
    let text = document
        .descendants()
        .find(|element| element.attribute("class") == Some("merman-foreignobject-fallback-text"))
        .unwrap();
    let viewport = text
        .ancestors()
        .find(|element| element.has_tag_name("svg"))
        .unwrap();
    assert_close(dimension(viewport, "width"), 120.0, "label area minimum");
    assert_close(dimension(viewport, "height"), 66.0, "label block height");
    assert_close(
        dimension(text, "x"),
        40.0,
        "text centers inside the authored block",
    );
    assert_close(
        dimension(text, "y"),
        10.5,
        "first line starts at the top of the block",
    );
    assert_eq!(text.text(), Some("short"));
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

#[test]
fn hourglass_fixture_labels_ignore_bounds_cached_before_the_label_was_cleared() {
    for layout_mode in LAYOUTS {
        for source in [
            include_str!(
                "../../../fixtures/flowchart/upstream_cypress_newshapes_spec_newshapessets_newshapesset1_tb_md_html_false_006.mmd"
            ),
            include_str!(
                "../../../fixtures/flowchart/upstream_cypress_newshapes_spec_newshapessets_newshapesset1_lr_md_html_false_054.mmd"
            ),
        ] {
            let (_, svg) = render(source, layout_mode);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let hourglass = document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("n44"))
                .expect("fixture hourglass node");
            let label = hourglass
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
                .expect("cleared fixture label");
            assert_eq!(
                label.attribute("transform"),
                Some("translate(0,0)"),
                "{layout_mode}: {source}"
            );
            assert!(
                !label.descendants().any(
                    |node| node.is_text() && !node.text().unwrap_or_default().trim().is_empty()
                )
            );
        }
    }
}

#[allow(dead_code)]
mod common;

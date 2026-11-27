use super::svg_emit::test_support::{FlowchartSvgModelRequest, render_flowchart_svg_model};
use super::*;
use crate::environment::RenderEnvironment;
use crate::model::FlowchartLayout;
use crate::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;

fn render_measured_shape(
    shape: &str,
    neo: bool,
    padding: f64,
    metrics: crate::text::TextMetrics,
) -> String {
    let look = if neo { "neo" } else { "classic" };
    let source = format!(
        "---\nconfig:\n  look: {look}\n  flowchart:\n    htmlLabels: true\n    minNodeWidth: 0\n    padding: {padding}\n---\nflowchart TD\nA@{{ shape: {shape}, label: Label }}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let render_context = parsed
        .render_context()
        .flowchart_render_context()
        .unwrap()
        .clone();
    let (metadata, semantic, _) = parsed.into_parts();
    let RenderSemanticModel::Flowchart(model) = semantic else {
        panic!("expected Flowchart");
    };
    let (width, height) =
        crate::flowchart::flowchart_node_render_dimensions(Some(shape), metrics, padding, neo);
    let layout = FlowchartLayout {
        nodes: vec![LayoutNode {
            id: "A".into(),
            x: 100.0,
            y: 100.0,
            width,
            height,
            is_cluster: false,
            label_width: Some(metrics.width),
            label_height: Some(metrics.height),
        }],
        edges: Vec::new(),
        edge_owners: Default::default(),
        clusters: Vec::new(),
        bounds: None,
        dom_node_order_by_root: std::collections::HashMap::from([(
            String::new(),
            vec!["A".into()],
        )]),
        route_contract: crate::graph_layout::RouteContract::DagreParity,
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    };
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let request = SvgRenderOptions {
        diagram_id: Some("brace-label-bounds".into()),
        ..SvgRenderOptions::default()
    };
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::unthemed_for_test(
        &request,
        &debug,
        &session,
        crate::DiagramFamilyId::FLOWCHART,
    )
    .unwrap();
    let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
    let prepared_theme =
        crate::flowchart::FlowchartPreparedTheme::resolve(&metadata.effective_config, None, None);
    let render_config = super::render_config::prepare_flowchart_render_config(
        &model,
        &metadata.effective_config,
        &prepared_theme.compatibility,
        layout.route_contract.is_elk_parity(),
        sidecar.base_typography(),
    );
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
        &model,
        &metadata.effective_config,
        false,
        execution.work_meter(),
    )
    .expect("edge style plan")
    .with_resolved_stroke_widths(
        &model,
        &prepared_theme.compatibility,
        execution.work_meter(),
    )
    .expect("prepared stroke widths")
    .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
    edge_style_plan
        .bind_terminal_typography(&render_config.text_style, false)
        .expect("terminal edge typography");
    let prepared_nodes = super::node_inventory::FlowchartPreparedNodes::prepare(
        &model,
        &render_context,
        &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
        layout.node_layout_view(),
        &sidecar,
        &metadata.effective_config,
        &prepared_theme,
        &render_config,
        session.work_meter(),
        true,
    )
    .expect("prepared nodes");
    render_flowchart_svg_model(
        FlowchartSvgModelRequest {
            render_config: &render_config,
            prepared_theme: &prepared_theme,
            prepared_nodes: &prepared_nodes,
            layout: &layout,
            swimlane_layout: None,
            model: &model,
            render_context: &render_context,
            effective_config: &metadata.effective_config,
            diagram_type: metadata.diagram_type.as_str(),
            diagram_title: None,

            edge_style_plan: &edge_style_plan,
            svg_label_sidecar: &sidecar,
        },
        &execution,
    )
    .unwrap()
    .to_string()
}

fn assert_neo_shape_viewport_contains_shifted_label(shape: &str) {
    for padding in [15.0, 100.0] {
        let metrics = crate::text::TextMetrics {
            width: 100.0,
            height: 20.0,
            line_count: 1,
        };
        let (width, height) =
            crate::flowchart::flowchart_node_render_dimensions(Some(shape), metrics, padding, true);
        let svg = render_measured_shape(shape, true, padding, metrics);
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        let viewbox: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let mut label_x = 0.0;
        let mut label_y = 0.0;
        for node in label.ancestors() {
            if let Some(transform) = node.attribute("transform") {
                let translation = transform
                    .strip_prefix("translate(")
                    .and_then(|value| value.strip_suffix(')'))
                    .expect("translation only");
                let values: Vec<f64> = translation
                    .split([',', ' '])
                    .filter(|value| !value.is_empty())
                    .map(|value| value.parse().unwrap())
                    .collect();
                label_x += values[0];
                label_y += values.get(1).copied().unwrap_or(0.0);
            }
        }
        let label_width: f64 = label.attribute("width").unwrap().parse().unwrap();
        let label_height: f64 = label.attribute("height").unwrap().parse().unwrap();
        let diagram_padding = 8.0;
        assert!(
            viewbox[0] <= label_x - diagram_padding + 1e-6,
            "{shape}, padding {padding}"
        );
        assert!(
            viewbox[1] <= label_y - diagram_padding + 1e-6,
            "{shape}, padding {padding}"
        );
        assert!(
            viewbox[0] + viewbox[2] >= label_x + label_width + diagram_padding - 1e-6,
            "{shape}, padding {padding}: label exceeds right viewport; viewbox={viewbox:?}, label=({label_x},{label_y},{label_width},{label_height})"
        );
        assert!(
            viewbox[1] + viewbox[3] >= label_y + label_height + diagram_padding - 1e-6,
            "{shape}, padding {padding}: label exceeds bottom viewport; viewbox={viewbox:?}, label=({label_x},{label_y},{label_width},{label_height})"
        );
        if shape == "documents" {
            // Pinned h=56, amplitude14: label offset(-10,-4), group y=-7.
            assert_eq!((label_x, label_y), (40.0, 86.0));
            let body = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("basic label-container outer-path"))
                .unwrap();
            assert_eq!(body.attribute("transform"), Some("translate(0,-7)"));
        } else if padding == 15.0 {
            // A contained label must not inflate the ordinary shape viewport.
            assert!((viewbox[2] - width - 2.0 * diagram_padding).abs() < 1e-6);
            assert!((viewbox[3] - height - 2.0 * diagram_padding).abs() < 1e-6);
        }
    }
}

#[test]
fn neo_brace_viewport_contains_shifted_labels_at_large_padding() {
    for shape in ["brace", "brace-r", "braces"] {
        assert_neo_shape_viewport_contains_shifted_label(shape);
    }
}

#[test]
fn neo_stacked_document_viewport_contains_source_positioned_label() {
    assert_neo_shape_viewport_contains_shifted_label("documents");
}
#[test]
fn wave_document_labels_use_source_padding_and_wave_offsets() {
    for (shape, neo) in [
        ("doc", false),
        ("doc", true),
        ("lin-doc", false),
        ("lin-doc", true),
    ] {
        let padding = 15.0;
        let svg = render_measured_shape(
            shape,
            neo,
            padding,
            crate::text::TextMetrics {
                width: 120.0,
                height: if shape == "doc" { 42.0 } else { 63.0 },
                line_count: 1,
            },
        );
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let label_width: f64 = label.attribute("width").unwrap().parse().unwrap();
        let label_height: f64 = label.attribute("height").unwrap().parse().unwrap();
        let padding_x = if neo { 16.0 } else { padding };
        let padding_y = if neo { 12.0 } else { padding };
        let w = label_width + 2.0 * padding_x;
        let h = label_height + 2.0 * padding_y;
        let wave_amplitude = if neo { h / 4.0 } else { h / 8.0 };
        let extension = if shape == "lin-doc" {
            (w / 2.0) * 0.1 / 2.0
        } else {
            0.0
        };
        let expected_x = -w / 2.0 + padding + extension;
        let expected_y = -h / 2.0 + padding - wave_amplitude;
        let transform = label
            .parent_element()
            .and_then(|node| node.attribute("transform"))
            .unwrap();
        let values: Vec<f64> = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .unwrap()
            .split([',', ' '])
            .filter(|value| !value.is_empty())
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(values.len(), 2, "{shape}, neo={neo}");
        assert!(
            (values[0] - expected_x).abs() < 1e-6,
            "{shape}, neo={neo}: values={values:?}, expected={expected_x}"
        );
        assert!(
            (values[1] - expected_y).abs() < 1e-6,
            "{shape}, neo={neo}: values={values:?}, expected={expected_y}"
        );
    }
}

#[test]
fn sloped_rectangle_label_uses_authored_padding_against_neo_geometry() {
    let padding = 15.0;
    let metrics = crate::text::TextMetrics {
        width: 120.0,
        height: 42.0,
        line_count: 1,
    };
    let svg = render_measured_shape("manual-input", true, padding, metrics);
    let doc = crate::xml_dom::Document::parse(&svg).unwrap();
    let label = doc
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    let label_width: f64 = label.attribute("width").unwrap().parse().unwrap();
    let label_height: f64 = label.attribute("height").unwrap().parse().unwrap();
    let w = label_width + 32.0;
    let h = label_height + 24.0;
    let transform = label
        .parent_element()
        .and_then(|node| node.attribute("transform"))
        .unwrap();
    let values: Vec<f64> = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap()
        .split([',', ' '])
        .filter(|value| !value.is_empty())
        .map(|value| value.parse().unwrap())
        .collect();
    assert!((values[0] - (-w / 2.0 + padding)).abs() < 1e-6);
    assert!((values[1] - (-h / 4.0 + padding)).abs() < 1e-6);
}

#[test]
fn neo_shape_svg_uses_source_vertices_rings_and_label_shift() {
    let metrics = crate::text::TextMetrics {
        width: 100.0,
        height: 20.0,
        line_count: 1,
    };
    for (shape, expected) in [
        (
            "lean-r",
            vec![(-17.5, 0.0), (130.0, 0.0), (147.5, -35.0), (0.0, -35.0)],
        ),
        (
            "lean-l",
            vec![(0.0, 0.0), (147.5, 0.0), (130.0, -35.0), (-17.5, -35.0)],
        ),
        (
            "trap-b",
            vec![(-17.5, 0.0), (147.5, 0.0), (130.0, -35.0), (0.0, -35.0)],
        ),
        (
            "trap-t",
            vec![(0.0, 0.0), (160.0, 0.0), (185.0, -50.0), (-25.0, -50.0)],
        ),
        (
            "hex",
            vec![
                (25.714285714285715, 0.0),
                (157.71428571428572, 0.0),
                (183.42857142857144, -45.0),
                (157.71428571428572, -90.0),
                (25.714285714285715, -90.0),
                (0.0, -45.0),
            ],
        ),
    ] {
        let svg = render_measured_shape(shape, true, 15.0, metrics);
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        let polygon = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("polygon") && node.attribute("class") == Some("label-container")
            })
            .unwrap();
        let actual: Vec<(f64, f64)> = polygon
            .attribute("points")
            .unwrap()
            .split_whitespace()
            .map(|point| {
                let (x, y) = point.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        assert_eq!(actual.len(), expected.len(), "{shape}");
        for (actual, expected) in actual.iter().zip(&expected) {
            // SVG formatting rounds coordinates; the pure source test retains full precision.
            assert!(
                (actual.0 - expected.0).abs() < 0.001 && (actual.1 - expected.1).abs() < 0.001,
                "{shape}: {actual:?} != {expected:?}"
            );
        }
    }
    for padding in [0.0, 15.0, 31.0] {
        let svg = render_measured_shape("odd", true, padding, metrics);
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        let body = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("basic label-container outer-path"))
            .unwrap();
        assert_eq!(body.attribute("transform"), Some("translate(5.5,0)"));
        // Exact RoughJS path bounds are covered by pure geometry tests; this DOM check
        // only guards the source label shift and final containment.
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            label.parent_element().unwrap().attribute("transform"),
            Some("translate(-44.5,-10)")
        );
        let viewbox: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let label_width: f64 = label.attribute("width").unwrap().parse().unwrap();
        let label_height: f64 = label.attribute("height").unwrap().parse().unwrap();
        assert!(viewbox[2] >= label_width + 16.0 && viewbox[3] >= label_height + 16.0);
    }
    let metrics = crate::text::TextMetrics {
        width: 40.0,
        height: 30.0,
        line_count: 1,
    };
    for (neo, padding, inner, outer) in [
        (false, 0.0, 25.0, 30.0),
        (false, 15.0, 40.0, 45.0),
        (false, 31.0, 56.0, 61.0),
        (true, 0.0, 41.0, 53.0),
        (true, 15.0, 41.0, 53.0),
        (true, 31.0, 41.0, 53.0),
    ] {
        let svg = render_measured_shape("dbl-circ", neo, padding, metrics);
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        for (class, expected) in [("inner-circle", inner), ("outer-circle", outer)] {
            let circle = doc
                .descendants()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap();
            let radius: f64 = circle.attribute("r").unwrap().parse().unwrap();
            assert_eq!(radius, expected, "neo={neo}, padding={padding}, {class}");
        }
        let viewbox: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(
            viewbox,
            vec![
                100.0 - outer - 8.0,
                100.0 - outer - 8.0,
                2.0 * outer + 16.0,
                2.0 * outer + 16.0
            ]
        );
    }
}

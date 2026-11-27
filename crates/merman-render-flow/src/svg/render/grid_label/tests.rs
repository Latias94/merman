use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use merman_model::{
    MermaidConfig, OperationControl,
    families::flowchart::{FlowchartModel, FlowchartRenderContext},
    family::DiagramFamilyId,
    presentation::PresentationConfig,
    resources::{InputResourcePolicy, ResourceProfile},
    runtime::RuntimePolicy,
};
use merman_render_core::{
    Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId},
    root_svg::RootViewportContext,
    svg_css::SvgId,
    svg_output::timing::RenderTiming,
    text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode},
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde_json::json;

use crate::{
    class_style::FlowchartClassDefinitions,
    layout_config::prepare_flowchart_render_config,
    node_inventory::{FlowchartNodeLayoutView, FlowchartPreparedNodes},
    projection::{FlowchartEdgeKey, FlowchartEdgeOwners},
    style::FlowchartEdgeStylePlan,
    svg::{
        FlowchartGridPaint, FlowchartSvgLayout, FlowchartSvgModelRequest, FlowchartSvgRoute,
        FlowchartSvgServices, GridLineHops, render_parity,
    },
    svg_label::FlowchartSvgLabelSidecar,
    theme::FlowchartPreparedTheme,
};

const RAW_EDGE_ID: &str = "repeat&\"edge";
const GRID_ROUTE: FlowchartSvgRoute<'static> = FlowchartSvgRoute::Grid(FlowchartGridPaint {
    curve: "linear",
    corner_radius: 0.0,
    line_hops: GridLineHops::Disabled,
});

struct BboxOffsetMeasurer(DeterministicTextMeasurer);

impl TextMeasurer for BboxOffsetMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0.measure(text, style)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width_px: Option<f64>,
        mode: WrapMode,
    ) -> TextMetrics {
        self.0.measure_wrapped(text, style, max_width_px, mode)
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        2.5
    }
}

fn render(
    diagram_id: &str,
    route: FlowchartSvgRoute<'_>,
    node_html: bool,
    policy: RenderResourcePolicy,
) -> Result<String> {
    let mut value = json!({
        "look": "classic", "securityLevel": "strict",
        "flowchart": {"htmlLabels": !node_html, "wrappingWidth": 90}
    });
    // When the root setting is absent, node labelHelper stays in HTML mode while ordinary
    // edge labels honor the deprecated flowchart-level SVG label setting.
    if !node_html {
        value["htmlLabels"] = json!(false);
    }
    let config = PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let model: FlowchartModel = serde_json::from_value(json!({
        "keyword": "flowchart", "direction": "LR",
        "nodes": [{"id":"A","label":"Start"}, {"id":"B","label":"End"}],
        "subgraphs": [{"id":"empty","title":"Empty cluster","nodes":[]}],
        "edges": [
            {"id":RAW_EDGE_ID,"from":"A","to":"B","length":1,"label":"First & safe",
             "style":["color:#304050","font-size:44px"]},
            {"id":RAW_EDGE_ID,"from":"A","to":"B","length":1,"label":"Second"},
            {"id":"missing","from":"A","to":"B","length":1,"label":"Unboxed"}
        ],
    }))
    .unwrap();
    let node = |id: &str, x, y, width, height, is_cluster| LayoutNode {
        id: id.to_owned(),
        x,
        y,
        width,
        height,
        is_cluster,
        label_width: Some(30.0),
        label_height: Some(14.0),
    };
    let nodes = [
        node("empty", 100.0, 130.0, 90.0, 42.0, true),
        node("A", 25.0, 50.0, 50.0, 38.0, false),
        node("B", 225.0, 50.0, 50.0, 38.0, false),
    ];
    let label = |x, y, width, height| LayoutLabel {
        x,
        y,
        width,
        height,
    };
    let clusters = [LayoutCluster {
        id: "empty".into(),
        x: 100.0,
        y: 130.0,
        width: 90.0,
        height: 42.0,
        padding: 8.0,
        diff: 0.0,
        offset_y: 0.0,
        title: "Empty cluster".into(),
        title_label: label(100.0, 118.0, 62.0, 14.0),
        requested_dir: None,
        effective_dir: "LR".into(),
        title_margin_top: 0.0,
        title_margin_bottom: 0.0,
    }];
    let edge = |index: usize, y, label| LayoutEdge {
        id: model.edges[index].id.clone(),
        from: "A".into(),
        to: "B".into(),
        points: vec![LayoutPoint { x: 50.0, y }, LayoutPoint { x: 200.0, y }],
        label,
        from_cluster: None,
        to_cluster: None,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: None,
        end_marker: None,
        stroke_dasharray: None,
    };
    // The boxes' centers differ from the shaft midpoints, and the layout rows reverse the
    // semantic occurrence order, even for the two edges with exactly the same semantic ID.
    let edges = [
        edge(2, 60.0, None),
        edge(1, 50.0, Some(label(173.0, 77.0, 40.0, 18.0))),
        edge(0, 40.0, Some(label(103.0, 22.0, 90.0, 42.0))),
    ];
    let owners = FlowchartEdgeOwners::new((0..3).rev().map(FlowchartEdgeKey::new).collect());
    let order = HashMap::from([(
        String::new(),
        nodes.iter().map(|node| node.id.clone()).collect(),
    )]);
    let layout = FlowchartSvgLayout {
        nodes: &nodes,
        edges: &edges,
        edge_owners: &owners,
        clusters: &clusters,
        dom_node_order_by_root: &order,
        route_contract: route,
    };
    let node_layout = FlowchartNodeLayoutView::Flowchart {
        nodes: &nodes,
        clusters: &clusters,
        dom_node_order_by_root: &order,
        edge_owners: &owners,
        solved_parents: None,
        dom: route.dom(),
        uses_projected_edges: route.uses_projected_edges(),
        canonical: false,
    };
    let context = FlowchartRenderContext::from_model(&model);
    let class_defs = FlowchartClassDefinitions::new(&model.class_defs);
    let theme = FlowchartPreparedTheme::resolve(&config, None, None);
    let render = prepare_flowchart_render_config(
        &model,
        &config,
        &theme.compatibility,
        route.is_elk_parity(),
        None,
    );
    assert_eq!(render.node_html_labels, node_html);
    assert!(!render.edge_html_labels);
    let labels = FlowchartSvgLabelSidecar::default();
    let work = Arc::new(OperationWorkMeter::new(policy));
    let label_rect = route.grid_paint().is_some();
    let styles = FlowchartEdgeStylePlan::prepare_with_class_definitions(
        &model,
        &config,
        label_rect,
        &class_defs,
        &work,
    )?
    .with_resolved_stroke_widths(&model, &theme.compatibility, &work)?
    .with_typography_work_meter(Arc::clone(&work));
    styles.bind_terminal_typography(&render.text_style, label_rect)?;
    let prepared_nodes = FlowchartPreparedNodes::prepare(
        &model,
        &context,
        &class_defs,
        node_layout,
        &labels,
        &config,
        &theme,
        &render,
        &work,
        true,
    )?;
    let request = FlowchartSvgModelRequest {
        render_config: &render,
        prepared_nodes: &prepared_nodes,
        prepared_theme: &theme,
        layout,
        swimlane_layout: None,
        model: &model,
        render_context: &context,
        effective_config: &config,
        diagram_type: "flowchart-v2",
        diagram_title: None,
        svg_label_sidecar: &labels,
        edge_style_plan: &styles,
    };
    let id = SvgId::new(diagram_id).unwrap();
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let svg = render_parity(
        request,
        &FlowchartSvgServices {
            diagram_id: id,
            root_viewport: RootViewportContext::new(DiagramFamilyId::FLOWCHART, id),
            operation: &operation,
            work_meter: &work,
            render_seed: operation.seed(),
            timing: RenderTiming::disabled(),
            measurer: &BboxOffsetMeasurer(DeterministicTextMeasurer::default()),
            math_renderer: None,
            icon_registry: None,
            defer_palette_stylesheet: false,
            native_canvas: None,
            canvas_background: None,
            native_family_theme: None,
            edge_trace: None,
        },
    )?;
    Ok(svg.as_str().to_owned())
}

fn attr(tag: &BytesStart<'_>, name: &str) -> Option<String> {
    let attribute = tag.try_get_attribute(name).unwrap()?;
    Some(
        quick_xml::escape::unescape(&attribute.value)
            .unwrap()
            .into_owned(),
    )
}

fn measured_helpers(svg: &str) -> (Vec<&str>, HashSet<String>) {
    let mut reader = Reader::from_str(svg);
    let mut ancestors = Vec::<(usize, BytesStart<'_>)>::new();
    let mut ids = HashSet::new();
    let mut helpers = Vec::new();
    let mut label_layers = 0;
    let mut empty_clusters = 0;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event().unwrap() {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let (tag, empty) = match event {
                    Event::Start(tag) => (tag, false),
                    Event::Empty(tag) => (tag, true),
                    _ => unreachable!(),
                };
                if let Some(id) = attr(&tag, "id") {
                    assert!(ids.insert(id.clone()), "duplicate id {id}");
                }
                if attr(&tag, "class").as_deref() == Some("edgeLabels") {
                    label_layers += 1;
                    assert!(empty);
                }
                if attr(&tag, "data-id").as_deref() == Some("empty") {
                    empty_clusters += 1;
                    assert_eq!(attr(&tag, "data-et").as_deref(), Some("cluster"));
                    assert_eq!(
                        attr(&ancestors.last().unwrap().1, "class").as_deref(),
                        Some("clusters")
                    );
                }
                if attr(&tag, "class").as_deref() == Some("label edgeLabel") {
                    assert_eq!(attr(&tag, "data-id").as_deref(), Some(RAW_EDGE_ID));
                    assert_eq!(attr(&tag, "data-et").as_deref(), Some("edge-label"));
                    let mut parents = ancestors.iter().rev().map(|(_, tag)| attr(tag, "class"));
                    assert_eq!(parents.next().flatten().as_deref(), Some("nodes"));
                    assert_eq!(parents.next().flatten().as_deref(), Some("root"));
                }
                if !empty {
                    ancestors.push((start, tag));
                }
            }
            Event::End(_) => {
                let (open, tag) = ancestors.pop().unwrap();
                if attr(&tag, "class").as_deref() == Some("label edgeLabel") {
                    helpers.push(&svg[open..reader.buffer_position() as usize]);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert!(ancestors.is_empty());
    assert_eq!(label_layers, 1);
    assert_eq!(empty_clusters, 1);
    (helpers, ids)
}

#[test]
fn grid_label_rects_use_measured_boxes_in_top_level_node_order() {
    for html in [true, false] {
        let svg = render(
            "grid-helpers",
            GRID_ROUTE,
            html,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let (helpers, _) = measured_helpers(&svg);
        let [first, second] = helpers.as_slice() else {
            panic!("expected two measured Grid helpers, got {}", helpers.len());
        };
        assert!(first.contains(r#"id="grid-helpers-merman-flowchart-document-edge-0-label""#));
        assert!(second.contains(r#"id="grid-helpers-merman-flowchart-document-edge-1-label""#));
        assert!(first.contains(r#"transform="translate(103, 22)"><rect width="0.1" height="0.1"/><g class="label" style="color:#304050""#));
        assert!(second.contains(
            r#"transform="translate(173, 77)"><rect width="0.1" height="0.1"/><g class="label""#
        ));
        assert!(!first.contains("44px"));
        assert!(second.contains("Second"));
        assert!(!svg.contains("Unboxed"));
        assert!(svg.find(r#"data-id="B""#).unwrap() < svg.find(first).unwrap());
        if html {
            assert!(first.contains(
                r#"transform="translate(-45, -21)"><rect/><foreignObject width="90" height="42""#
            ));
            assert!(
                first
                    .contains("color: rgb(48, 64, 80); display: table; white-space: break-spaces;")
            );
            assert!(first.contains(
                r#"<span class="nodeLabel" style="color:#304050"><p>First &amp; safe</p></span>"#
            ));
            assert!(second.contains(
                r#"transform="translate(-20, -9)"><rect/><foreignObject width="40" height="18""#
            ));
            assert!(second.contains("display: table-cell; white-space: nowrap;"));
        } else {
            assert!(first.contains(r#"transform="translate(-45, -23.5)"><rect/><g><rect class="background" style="stroke: none"/><text y="-10.1" style="fill:#304050""#));
            assert!(second.contains(r#"transform="translate(-20, -11.5)""#));
            let (_, text) = first.split_once("<text ").unwrap();
            assert!(text.contains("&amp;"), "{text}");
            assert!(!first.contains("<foreignObject"));
            assert!(!first.contains("text-anchor="));
        }
    }
}

#[test]
fn grid_label_rect_ids_stay_in_their_document_namespace() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let first = render("grid-helpers", GRID_ROUTE, true, policy.clone()).unwrap();
    let second = render("other-grid-helpers", GRID_ROUTE, true, policy).unwrap();
    let (_, first_ids) = measured_helpers(&first);
    let (_, second_ids) = measured_helpers(&second);
    assert!(first_ids.contains("grid-helpers-merman-flowchart-document-edge-0-label"));
    assert!(second_ids.contains("other-grid-helpers-merman-flowchart-document-edge-0-label"));
    assert!(first_ids.is_disjoint(&second_ids));
}

#[test]
fn grid_label_rects_share_the_svg_byte_ceiling() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let svg = render("grid-helpers", GRID_ROUTE, true, policy.clone()).unwrap();
    let with_bytes = |bytes| {
        policy
            .clone()
            .with_limit(ResourceLimitId::MaxSvgBytes, bytes)
            .unwrap()
    };
    assert_eq!(
        render("grid-helpers", GRID_ROUTE, true, with_bytes(svg.len())).unwrap(),
        svg
    );
    assert!(matches!(
        render("grid-helpers", GRID_ROUTE, true, with_bytes(svg.len() - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));
}

#[test]
fn elk_keeps_ordinary_labels_in_its_flat_edge_label_group() {
    let svg = render(
        "grid-helpers",
        FlowchartSvgRoute::ElkParity,
        false,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .unwrap();
    assert!(svg.contains(r#"<g class="edgeLabels"><g class="edgeLabel""#));
    assert!(svg.contains("Unboxed"));
    assert!(!svg.contains(r#"<g class="label edgeLabel""#));
}

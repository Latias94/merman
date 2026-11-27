use merman_render_flow::svg::{self, FlowchartSvgModelRequest};

use super::*;

fn flowchart_request<'a>(
    artifact: &'a crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &'a merman_model::presentation::PresentationMetadata,
) -> Result<FlowchartSvgModelRequest<'a>> {
    Ok(FlowchartSvgModelRequest {
        layout: svg_layout(artifact.pair().layout()),
        swimlane_layout: None,
        model: artifact.pair().semantic(),
        render_context: artifact.render_context(),
        effective_config: &metadata.effective_config,
        diagram_type: metadata.diagram_type.as_str(),
        diagram_title: metadata.title.as_deref(),
        svg_label_sidecar: artifact.svg_label_sidecar(),
        edge_style_plan: artifact.edge_style_plan(),
        prepared_theme: artifact.prepared_theme(),
        prepared_nodes: artifact.prepared_nodes()?,
        render_config: artifact.render_config(),
    })
}

pub(in crate::svg::parity) fn render_flowchart_svg_artifact(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_model::presentation::PresentationMetadata,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let request = flowchart_request(artifact, metadata)?;
    svg::render_parity(request, &svg_services(options))
}

pub(in crate::svg::parity) fn flowchart_edge_geometry(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_model::presentation::PresentationMetadata,
    options: &SvgExecution<'_>,
) -> Result<Vec<EdgePaintGeometry>> {
    let request = flowchart_request(artifact, metadata)?;
    svg::edge_geometry(request, &svg_services(options))
}

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
pub(in crate::svg::parity) fn render_native_compact_flowchart_artifact(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_model::presentation::PresentationMetadata,
    execution: &SvgExecution<'_>,
) -> Result<String> {
    let request = flowchart_request(artifact, metadata)?;
    let debug = execution.debug;
    let standard_debug = debug.include_edges
        && debug.include_nodes
        && debug.include_clusters
        && !debug.include_cluster_debug_markers
        && !debug.include_edge_id_labels
        && !debug.include_timing_diagnostics
        && debug.flowchart_edge_trace.is_none();
    let services = svg_services(execution);
    let trusted_theme_css = execution.family.session().allows_trusted_theme_css();
    svg::render_native_compact(request, &services, standard_debug, trusted_theme_css)
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    #[derive(Clone, Copy)]
    pub(in crate::svg::parity::flowchart) struct FlowchartSvgModelRequest<'a> {
        pub layout: &'a FlowchartLayout,
        pub swimlane_layout: Option<&'a SwimlaneLayout>,
        pub model: &'a crate::flowchart::FlowchartModel,
        pub render_context: &'a crate::flowchart::FlowchartRenderContext,
        pub effective_config: &'a merman_model::presentation::PresentationConfig,
        pub diagram_type: &'a str,
        pub diagram_title: Option<&'a str>,
        pub svg_label_sidecar: &'a crate::flowchart::FlowchartSvgLabelSidecar,
        pub edge_style_plan: &'a FlowchartEdgeStylePlan,
        pub prepared_theme: &'a crate::flowchart::FlowchartPreparedTheme,
        pub prepared_nodes: &'a FlowchartPreparedNodes,
        pub render_config: &'a FlowchartRenderConfig,
    }

    impl<'a> From<FlowchartSvgModelRequest<'a>> for svg::FlowchartSvgModelRequest<'a> {
        fn from(request: FlowchartSvgModelRequest<'a>) -> Self {
            Self {
                layout: svg_layout(request.layout),
                swimlane_layout: request.swimlane_layout.map(swimlane_svg_layout),
                model: request.model,
                render_context: request.render_context,
                effective_config: request.effective_config,
                diagram_type: request.diagram_type,
                diagram_title: request.diagram_title,
                svg_label_sidecar: request.svg_label_sidecar,
                edge_style_plan: request.edge_style_plan,
                prepared_theme: request.prepared_theme,
                prepared_nodes: request.prepared_nodes,
                render_config: request.render_config,
            }
        }
    }

    pub(in crate::svg::parity::flowchart) fn render_flowchart_svg_model(
        request: FlowchartSvgModelRequest<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<root_svg::RootedSvg> {
        svg::render_parity(request.into(), &svg_services(options))
    }
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod integration_tests {
    use super::test_support::{FlowchartSvgModelRequest, render_flowchart_svg_model};
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::model::{FlowchartLayout, LayoutNode};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use merman_render_flow::projection::{FlowchartEdgeKey, prepare_flowchart_render_inputs};

    #[test]
    fn unmerged_self_loop_occurrences_keep_source_owner_label_and_first_slot() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA -->|first| A\nA -->|regular| B\nA -->|last| A\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let render_context = parsed
            .render_context()
            .flowchart_render_context()
            .unwrap()
            .clone();
        let (_, semantic, _) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(mut model) = semantic else {
            panic!("expected Flowchart model");
        };
        model.keyword = "agentflow-beta".to_owned();
        // Authored ids may overlap generated helper spellings without sharing an owner.
        model.edges[1].id = "A-cyclic-special-1".to_owned();
        let owners = crate::flowchart::FlowchartEdgeOwners::new(vec![
            FlowchartEdgeKey::self_loop_segment(2, 0),
            FlowchartEdgeKey::self_loop_segment(2, 1),
            FlowchartEdgeKey::self_loop_segment(2, 2),
            FlowchartEdgeKey::new(1),
        ]);
        let layout_edges = [
            "A-cyclic-special-1",
            "A-cyclic-special-mid",
            "A-cyclic-special-2",
            &model.edges[1].id,
        ]
        .map(|id| {
            serde_json::from_value(serde_json::json!({
                "id": id, "from": "A", "to": "A", "points": []
            }))
            .unwrap()
        });
        let coverage = owners.validate(&layout_edges, &model.edges).unwrap();
        let inputs =
            prepare_flowchart_render_inputs(&model, &render_context, &owners, coverage, false);
        let render_model = crate::flowchart::FlowchartRenderModelRef::new(&model, &render_context);
        assert_eq!(
            inputs
                .render_edges
                .iter()
                .map(|edge| edge.key)
                .collect::<Vec<_>>(),
            owners.iter().collect::<Vec<_>>()
        );
        assert_eq!(inputs.extra_nodes.len(), 2);
        let labels = inputs
            .render_edges
            .iter()
            .map(|edge| render_model.edge_label_for_occurrence(edge.key, &edge.edge))
            .collect::<Vec<_>>();
        assert_eq!(labels, [None, Some("last"), None, Some("regular")]);
        assert_eq!(
            inputs.render_edges[0].edge.id,
            inputs.render_edges[3].edge.id
        );
        assert_ne!(inputs.render_edges[0].key, inputs.render_edges[3].key);
    }

    #[test]
    fn diagram_id_terminal_precedes_later_flowchart_node_emission_error() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA@{ img: \"https://example.invalid/a.svg\", label: \"A\" }\n",
                ParseOptions::strict(),
            )
            .expect("parse succeeds")
            .expect("detects Flowchart")
            .into_presentation();
        let render_context = parsed
            .render_context()
            .flowchart_render_context()
            .expect("Flowchart render context")
            .clone();
        let (metadata, semantic, _) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(mut model) = semantic else {
            panic!("expected Flowchart model");
        };
        let node = model.nodes.first_mut().expect("fixture node");
        assert_eq!(node.layout_shape.as_deref(), Some("imageSquare"));
        node.img = None;

        let layout = FlowchartLayout {
            nodes: vec![LayoutNode {
                id: node.id.clone(),
                x: 40.0,
                y: 30.0,
                width: 80.0,
                height: 60.0,
                is_cluster: false,
                label_width: Some(10.0),
                label_height: Some(10.0),
            }],
            edges: Vec::new(),
            edge_owners: Default::default(),
            clusters: Vec::new(),
            bounds: None,
            dom_node_order_by_root: std::collections::HashMap::from([(
                String::new(),
                vec![node.id.clone()],
            )]),
            route_contract: crate::graph_layout::RouteContract::DagreParity,
            placement: Default::default(),
            uses_elk_adapter_dom: false,
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .expect("valid SVG byte limit");
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions {
            diagram_id: Some("terminal".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::FLOWCHART,
        )
        .expect("SVG execution");
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            &metadata.effective_config,
            None,
            None,
        );
        let render_config = prepare_flowchart_render_config(
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
        let prepared_nodes = FlowchartPreparedNodes::prepare(
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

        let error = render_flowchart_svg_model(
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
                diagram_title: metadata.title.as_deref(),
                edge_style_plan: &edge_style_plan,
                svg_label_sidecar: &sidecar,
            },
            &execution,
        )
        .expect_err("diagram-ID rejection must stop before the invalid image node is emitted");

        let crate::Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte rejection, got {error}");
        };
        assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_terminal_straightening_reaches_svg_and_preserves_ports() {
        use crate::model::{LayoutEdge, LayoutPoint};
        use base64::Engine as _;

        // Pinned Mermaid geometry.spec.ts terminal-jog case, with measured rectangles whose
        // boundaries coincide with its ports. The layout is fixed to isolate SVG postprocessing.
        let render = |enabled: bool, trace: bool| {
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(
                    &format!("---\nconfig:\n  elk:\n    straightenEdges: {enabled}\n---\nflowchart LR\nA --> B"),
                    ParseOptions::strict(),
                ).unwrap().unwrap().into_presentation();
            let render_context = parsed
                .render_context()
                .flowchart_render_context()
                .unwrap()
                .clone();
            let (metadata, semantic, _) = parsed.into_parts();
            let RenderSemanticModel::Flowchart(model) = semantic else {
                panic!("Flowchart");
            };
            let edge_id = model.edges[0].id.clone();
            let layout = FlowchartLayout {
                nodes: [("A", 153.0, 116.25), ("B", 400.0, 320.0)]
                    .into_iter()
                    .map(|(id, x, y)| LayoutNode {
                        id: id.into(),
                        x,
                        y,
                        width: 80.0,
                        height: 40.0,
                        is_cluster: false,
                        label_width: Some(10.0),
                        label_height: Some(10.0),
                    })
                    .collect(),
                edges: vec![LayoutEdge {
                    id: edge_id.clone(),
                    from: "A".into(),
                    to: "B".into(),
                    from_cluster: None,
                    to_cluster: None,
                    points: [
                        (193.0, 116.25),
                        (218.0, 116.25),
                        (218.0, 119.5),
                        (400.0, 119.5),
                        (400.0, 300.0),
                    ]
                    .into_iter()
                    .map(|(x, y)| LayoutPoint { x, y })
                    .collect(),
                    label: None,
                    start_label_left: None,
                    start_label_right: None,
                    end_label_left: None,
                    end_label_right: None,
                    start_marker: None,
                    end_marker: None,
                    stroke_dasharray: None,
                }],
                edge_owners: crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]),
                clusters: Vec::new(),
                bounds: None,
                dom_node_order_by_root: std::collections::HashMap::from([(
                    String::new(),
                    vec!["A".into(), "B".into()],
                )]),
                route_contract: crate::graph_layout::RouteContract::ElkParity,
                placement: Default::default(),
                uses_elk_adapter_dom: true,
            };
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let request = SvgRenderOptions {
                diagram_id: Some("terminal-jog".into()),
                ..SvgRenderOptions::default()
            };
            let debug = if trace {
                SvgDebugOptions::default().with_flowchart_edge_trace(
                    edge_id,
                    crate::svg::FlowchartEdgeTraceCollector::default(),
                )
            } else {
                SvgDebugOptions::default()
            };
            let execution = SvgExecution::unthemed_for_test(
                &request,
                &debug,
                &session,
                crate::DiagramFamilyId::FLOWCHART,
            )
            .unwrap();
            let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
            let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
                &metadata.effective_config,
                None,
                None,
            );
            let render_config = prepare_flowchart_render_config(
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
            let prepared_nodes = FlowchartPreparedNodes::prepare(
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
        };
        let route = |svg: &str| {
            let doc = crate::xml_dom::Document::parse(svg).unwrap();
            let path = doc
                .descendants()
                .find(|n| n.attribute("data-points").is_some())
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            (
                path.attribute("d").unwrap().to_owned(),
                points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            )
        };
        let off = render(false, false);
        let on = render(true, false);
        let (old_path, old_points) = route(&off);
        let (new_path, new_points) = route(&on);
        assert_eq!(old_points.len(), 5);
        assert_eq!(new_points.len(), 3);
        assert_eq!(old_points.first(), new_points.first());
        assert_eq!(old_points.last(), new_points.last());
        assert_eq!(new_points[0].1, new_points[1].1);
        assert_ne!(old_path, new_path);
        assert_eq!(
            on,
            render(true, true),
            "diagnostics must preserve processed geometry"
        );
    }
}

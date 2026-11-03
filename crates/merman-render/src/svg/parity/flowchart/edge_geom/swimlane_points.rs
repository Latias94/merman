#[cfg(test)]
mod tests {
    use crate::model::LayoutPoint;

    fn coordinates(points: &[LayoutPoint]) -> Vec<(f64, f64)> {
        points.iter().map(|point| (point.x, point.y)).collect()
    }

    #[test]
    #[cfg(feature = "diagram-swimlane")]
    fn actual_swimlane_renderer_clips_two_point_routes_when_config_names_elk() {
        use crate::environment::RenderEnvironment;
        use crate::model::{FlowchartLayout, LayoutNode, SwimlaneDirection, SwimlaneLayout};
        use crate::svg::parity::flowchart::node_inventory::{
            FlowchartNodeLayoutView, FlowchartPreparedNodes,
        };
        use crate::svg::parity::flowchart::render_config::{
            flowchart_node_label_fill_config_override, prepare_flowchart_render_config,
        };
        use crate::svg::parity::flowchart::svg_emit::FlowchartSvgModelRequest;
        use crate::svg::{SvgDebugOptions, SvgExecution, SvgRenderOptions};
        use merman_core::{MermaidEngine, ParseOptions};
        use merman_model::presentation::RenderSemanticModel;

        // Mermaid 12.2 edges.spec.js supplies a two-point swimlane route while
        // getConfig().layout is ELK. Use real rectangle intersections here.
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"layout\": \"elk\"}}%%\nswimlane-beta TB\nA --> B\n",
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
        let (metadata, semantic, _) = parsed.into_parts();
        assert_eq!(metadata.effective_config.get_str("layout"), Some("elk"));
        let RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("expected Flowchart semantic model");
        };
        let edge = &model.edges[0];
        let layout = FlowchartLayout {
            nodes: [("A", 0.0), ("B", 100.0)]
                .into_iter()
                .map(|(id, y)| LayoutNode {
                    id: id.to_string(),
                    x: 0.0,
                    y,
                    width: 40.0,
                    height: 20.0,
                    is_cluster: false,
                    label_width: None,
                    label_height: None,
                })
                .collect(),
            edges: vec![
                serde_json::from_value(serde_json::json!({
                    "id": edge.id,
                    "from": edge.from,
                    "to": edge.to,
                    "points": [{"x": 0.0, "y": 5.0}, {"x": 0.0, "y": 95.0}],
                    "label": null
                }))
                .unwrap(),
            ],
            edge_owners: crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]),
            clusters: Vec::new(),
            bounds: None,
            dom_node_order_by_root: std::collections::HashMap::from([(
                String::new(),
                vec!["A".to_string(), "B".to_string()],
            )]),
            uses_elk_adapter_dom: false,
        };
        let swimlane_layout = SwimlaneLayout {
            direction: SwimlaneDirection::Tb,
            nodes: Vec::new(),
            lanes: Vec::new(),
            edges: Vec::new(),
            edge_owners: Default::default(),
            bounds: None,
        };
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let options = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &options,
            &debug,
            &session,
            crate::DiagramFamilyId::SWIMLANE,
        )
        .unwrap();
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &metadata.effective_config,
            flowchart_node_label_fill_config_override(&metadata.effective_config),
            execution.work_meter(),
        )
        .expect("prepared theme");
        let edge_theme =
            crate::flowchart::FlowchartEdgeThemeStyle::resolve(None, execution.work_meter())
                .expect("prepared edge theme");
        let render_config = prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &prepared_theme.compatibility,
            layout.uses_elk_adapter_dom,
            sidecar.base_typography(),
            sidecar.edge_label_padding(),
        );
        let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
            &model,
            &metadata.effective_config,
            true,
            execution.work_meter(),
        )
        .expect("edge style plan")
        .with_resolved_stroke_widths(
            &model,
            &edge_theme,
            &metadata.effective_config,
            &prepared_theme.compatibility,
            execution.work_meter(),
        )
        .expect("prepared stroke widths")
        .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
        edge_style_plan
            .bind_terminal_typography(&render_config.text_style, true)
            .expect("terminal Swimlane edge typography");
        let prepared_nodes = FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &metadata.effective_config,
            &prepared_theme,
            &render_config,
            session.work_meter(),
        )
        .expect("prepared fixed-layout nodes");
        let theme_evidence = crate::flowchart::FlowchartThemeEvidenceRecorder::default();
        let effect_evidence = crate::diagram_theme::SvgShadowEvidenceRecorder::default();
        let expected_effect_applications = std::cell::Cell::new(0);
        let request = FlowchartSvgModelRequest {
            render_config: &render_config,
            prepared_nodes: &prepared_nodes,
            prepared_theme: &prepared_theme,
            layout: &layout,
            swimlane_layout: Some(&swimlane_layout),
            model: &model,
            render_context: &render_context,
            effective_config: &metadata.effective_config,
            diagram_type: metadata.diagram_type.as_str(),
            diagram_title: None,
            svg_label_sidecar: &sidecar,
            theme_evidence: &theme_evidence,
            effect_evidence: &effect_evidence,
            expected_effect_applications: &expected_effect_applications,
            edge_style_plan: &edge_style_plan,
            edge_theme: &edge_theme,
        };
        let painted_edges = super::super::paint::edge_geometry(request, &execution).unwrap();
        assert_eq!(
            coordinates(painted_edges[0].points.as_ref().unwrap()),
            [(0.0, 10.0), (0.0, 90.0)]
        );
        let svg = super::super::svg_emit::render_flowchart_svg_model(request, &execution).unwrap();
        let document = crate::xml_dom::Document::parse(svg.as_str()).unwrap();
        let encoded = document
            .descendants()
            .find_map(|node| node.attribute("data-points"))
            .unwrap();
        use base64::Engine as _;
        let points: Vec<LayoutPoint> = serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            coordinates(&points),
            coordinates(painted_edges[0].points.as_ref().unwrap())
        );
    }
}

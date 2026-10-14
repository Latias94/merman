//! Mermaid's swimlane endpoint clipping from rendering-elements/edges.js.

use crate::model::LayoutPoint;

const PIN_TOLERANCE: f64 = 0.5;

fn coincides(first: &LayoutPoint, second: &LayoutPoint) -> bool {
    (first.x - second.x).abs() < PIN_TOLERANCE && (first.y - second.y).abs() < PIN_TOLERANCE
}

pub(super) fn clip_swimlane_endpoints(
    points: &[LayoutPoint],
    mut tail_intersect: impl FnMut(&LayoutPoint) -> LayoutPoint,
    mut head_intersect: impl FnMut(&LayoutPoint) -> LayoutPoint,
    out: &mut Vec<LayoutPoint>,
) {
    out.clear();
    out.reserve(points.len());
    if points.len() < 2 {
        out.extend_from_slice(points);
        return;
    }
    if points.len() == 2 {
        out.push(tail_intersect(&points[0]));
        out.push(head_intersect(&points[1]));
        return;
    }

    let inner = &points[1..points.len() - 1];
    let first_inner = &inner[0];
    let last_inner = &inner[inner.len() - 1];
    let new_first = tail_intersect(first_inner);
    let new_last = if coincides(&points[points.len() - 1], last_inner) {
        last_inner.clone()
    } else {
        head_intersect(last_inner)
    };
    if !coincides(&new_first, first_inner) {
        out.push(new_first);
    }
    out.extend_from_slice(inner);
    if !coincides(&new_last, last_inner) {
        out.push(new_last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn coordinates(points: &[LayoutPoint]) -> Vec<(f64, f64)> {
        points.iter().map(|point| (point.x, point.y)).collect()
    }

    #[test]
    fn two_point_swimlane_route_clips_each_endpoint_without_dropping_the_route() {
        let points = [point(0.0, 0.0), point(0.0, 100.0)];
        let mut out = Vec::new();
        clip_swimlane_endpoints(&points, Clone::clone, Clone::clone, &mut out);
        assert_eq!(coordinates(&out), [(0.0, 0.0), (0.0, 100.0)]);
    }

    #[test]
    fn duplicated_end_pin_skips_polygon_intersection() {
        // Mermaid edges.spec.js: honor the exact port prepared by the layout core.
        let points = [
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(0.0, 20.0),
            point(-100.0, 20.0),
            point(-100.0, 53.0),
            point(-100.0, 53.0),
        ];
        let mut out = Vec::new();
        clip_swimlane_endpoints(
            &points,
            Clone::clone,
            |_| panic!("pinned end must not be intersected again"),
            &mut out,
        );
        assert_eq!(
            coordinates(&out),
            [(0.0, 0.0), (0.0, 20.0), (-100.0, 20.0), (-100.0, 53.0)]
        );
    }

    #[test]
    fn source_pin_is_clipped_to_the_rendered_shape() {
        let points = [
            point(10.0, 14.0),
            point(10.0, 14.0),
            point(10.0, 60.0),
            point(90.0, 60.0),
        ];
        let mut out = Vec::new();
        clip_swimlane_endpoints(
            &points,
            |inner| {
                assert_eq!((inner.x, inner.y), (10.0, 14.0));
                point(8.0, 12.0)
            },
            Clone::clone,
            &mut out,
        );
        assert_eq!(coordinates(&out), [(8.0, 12.0), (10.0, 14.0), (10.0, 60.0)]);
    }

    #[test]
    fn pin_tolerance_is_strict_and_checks_both_coordinates() {
        for (dx, dy) in [(0.49, 0.49), (0.5, 0.49), (0.49, 0.5), (0.5, 0.5)] {
            let points = [
                point(0.0, 0.0),
                point(0.0, 20.0),
                point(100.0, 20.0),
                point(100.0 + dx, 20.0 + dy),
            ];
            let mut out = Vec::new();
            let mut calls = 0;
            clip_swimlane_endpoints(
                &points,
                |inner| point(inner.x + dx, inner.y + dy),
                |_| {
                    calls += 1;
                    point(101.0, 21.0)
                },
                &mut out,
            );
            let pinned = dx < 0.5 && dy < 0.5;
            assert_eq!(calls, usize::from(!pinned));
            assert_eq!(out.len(), if pinned { 2 } else { 4 });
        }
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
        use crate::svg::parity::flowchart::svg_emit::{
            FlowchartSvgModelRequest, render_flowchart_svg_model,
        };
        use crate::svg::{SvgDebugOptions, SvgExecution, SvgRenderOptions};
        use merman_core::{Engine, ParseOptions, RenderSemanticModel};

        // Mermaid 12.2 edges.spec.js supplies a two-point swimlane route while
        // getConfig().layout is ELK. Use real rectangle intersections here.
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"layout\": \"elk\"}}%%\nswimlane-beta TB\nA --> B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let render_context = parsed.flowchart_render_context().unwrap().clone();
        let (metadata, semantic) = parsed.into_parts();
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
        let mut painted_edges = Vec::new();
        render_flowchart_svg_model(
            FlowchartSvgModelRequest {
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
            },
            &execution,
            Some(&mut painted_edges),
        )
        .unwrap();
        assert_eq!(
            coordinates(painted_edges[0].points.as_ref().unwrap()),
            [(0.0, 10.0), (0.0, 90.0)]
        );
    }
}

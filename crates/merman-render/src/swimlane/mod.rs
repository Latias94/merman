mod bounds;
mod config;
mod direction;
mod geometry;
mod prepare;
mod routing;
mod sugiyama;
mod work_budget;
mod working;

use crate::Result;
use crate::flowchart::FlowchartConfigView;
use crate::math::MathRenderer;
use crate::model::{
    Bounds, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneLayout, SwimlaneNodeLayout,
};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::diagrams::flowchart::{FlowchartModel, FlowchartRenderContext};
use std::cmp::Ordering;
use std::sync::Arc;

/// Compare identifiers by their UTF-16 code units.
///
/// This matches JavaScript's relational string ordering while remaining independent of host
/// locale and bundled collation data. Every Swimlane algorithmic tie-break uses this comparator so
/// hash-map iteration order cannot affect cycle removal, layering, or lane ordering.
pub(crate) fn deterministic_identifier_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn output_bounds(layout: &working::WorkingLayout) -> Option<Bounds> {
    let mut points = Vec::new();
    for node in layout
        .nodes
        .values()
        .filter(|node| node.kind != working::WorkingNodeKind::Dummy)
    {
        let width = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_width
        } else {
            node.width
        };
        let height = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_height
        } else {
            node.height
        };
        points.push((node.x - width / 2.0, node.y - height / 2.0));
        points.push((node.x + width / 2.0, node.y + height / 2.0));
    }
    for edge in &layout.original_edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
    }
    Bounds::from_points(points)
}

/// Lays out a Swimlane model under the resource policy owned by the render operation.
pub(crate) fn layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&crate::flowchart::FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    let source_nodes = model.nodes.len().saturating_add(model.subgraphs.len());
    let source_edges = model.edges.len();
    work_meter.preflight(swimlane_layout_preflight_work_units(
        source_nodes,
        source_edges,
    ))?;
    work_meter.charge(swimlane_core_layout_work_units(source_nodes, source_edges))?;
    let config = config::SwimlaneConfig::from_config(effective_config);
    let mut working = prepare::prepare(
        model,
        render_label_sources,
        effective_config,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
    )?;
    let reversed = sugiyama::run(&mut working, config);
    for edge in &mut working.original_edges {
        edge.reversed_for_layout = reversed.contains(&edge.id);
    }
    bounds::assign_canonical_group_bounds(&mut working);
    let mut work_budget = work_budget::LayoutWorkBudget::for_operation(work_meter);
    routing::route(&mut working, &mut work_budget)?;
    direction::post_process(&mut working, &mut work_budget)?;

    // Mermaid's swimlane core only normalizes the implicit `basis` curve to
    // `rounded`; an explicit edge/default/config curve remains authoritative.
    // Resolve the same precedence here before the layout artifact is consumed
    // by the SVG renderer.
    let config_curve = FlowchartConfigView::new(effective_config.as_value()).render_curve();
    let default_curve = model
        .edge_defaults
        .as_ref()
        .and_then(|defaults| defaults.interpolate.as_deref())
        .filter(|curve| !curve.is_empty())
        .or(config_curve.as_deref())
        .unwrap_or("basis");
    let curve_by_owner = model
        .edges
        .iter()
        .map(|edge| {
            let curve = edge
                .interpolate
                .as_deref()
                .filter(|curve| !curve.is_empty())
                .unwrap_or(default_curve);
            curve
        })
        .collect::<Vec<_>>();

    if working.original_edges.len() != model.edges.len() {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "Swimlane edge-owner count {} does not match semantic edge count {}",
                working.original_edges.len(),
                model.edges.len()
            ),
        });
    }

    let bounds = output_bounds(&working);
    let nodes = working
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.kind,
                working::WorkingNodeKind::Content | working::WorkingNodeKind::EdgeLabel
            )
        })
        .map(|node| SwimlaneNodeLayout {
            id: node.id.clone(),
            label: node.label.clone(),
            label_type: node.label_type.clone(),
            shape: node.shape.clone(),
            parent_id: node.parent_id.clone(),
            top_lane_id: node.top_lane_id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            label_width: node.label_width,
            label_height: node.label_height,
            layer: node.layer,
            order: node.order,
            is_edge_label: node.kind == working::WorkingNodeKind::EdgeLabel,
        })
        .collect();
    let lanes = working
        .nodes
        .values()
        .filter(|node| node.kind == working::WorkingNodeKind::Group)
        .map(|lane| SwimlaneLaneLayout {
            id: lane.id.clone(),
            title: lane.label.clone(),
            parent_id: lane.parent_id.clone(),
            x: lane.x,
            y: lane.y,
            width: lane.width,
            height: lane.height,
            padding: lane.padding,
            title_label_width: lane.label_width,
            title_label_height: lane.label_height,
            content_top: lane.content_top,
            title_rect: lane.title_rect.clone(),
            requested_dir: lane.requested_dir.clone(),
        })
        .collect();
    let mut edge_owners = Vec::with_capacity(working.original_edges.len());
    let transport_plan =
        crate::flowchart::FlowchartEdgeTransportPlan::for_semantic_edges(&model.edges);
    let edges = working
        .original_edges
        .iter()
        .enumerate()
        .map(|(semantic_index, edge)| {
            let key = crate::flowchart::FlowchartEdgeKey::new(semantic_index);
            let semantic_edge = &model.edges[semantic_index];
            let expected_transport_id =
                transport_plan
                    .id(key)
                    .ok_or_else(|| crate::Error::InvalidModel {
                        message: format!(
                            "missing Swimlane transport id for semantic edge owner {semantic_index}"
                        ),
                    })?;
            if edge.id != expected_transport_id || edge.reference_id != expected_transport_id {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Swimlane working edge `{}` is not bound to semantic owner {}",
                        edge.id, semantic_index
                    ),
                });
            }
            if edge.from != semantic_edge.from || edge.to != semantic_edge.to {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Swimlane working edge `{}` endpoints do not match semantic owner {}",
                        edge.id, semantic_index
                    ),
                });
            }
            edge_owners.push(key);
            Ok(SwimlaneEdgeLayout {
                id: semantic_edge.id.clone(),
                from: semantic_edge.from.clone(),
                to: semantic_edge.to.clone(),
                points: edge.points.clone(),
                label_node_id: edge.label_node_id.clone(),
                reversed_for_layout: edge.reversed_for_layout,
                curve: match curve_by_owner
                    .get(semantic_index)
                    .copied()
                    .unwrap_or("basis")
                {
                    "basis" => "rounded",
                    curve => curve,
                }
                .to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(SwimlaneLayout {
        direction: working.direction,
        nodes,
        lanes,
        edges,
        edge_owners: crate::flowchart::FlowchartEdgeOwners::new(edge_owners),
        bounds,
    })
}

fn swimlane_core_layout_work_units(nodes: usize, edges: usize) -> usize {
    let baseline = nodes
        .saturating_mul(4)
        .saturating_add(edges.saturating_mul(5));

    // This is the stable, family-accounted cost for prepare, Sugiyama, and routing. The linear
    // baseline covers their source-item passes plus occurrence-identity preparation. Routing adds
    // edges incrementally and can compare each new route with every earlier route, so charge one
    // conservative unit per unordered source-edge pair. Direction post-processing and SVG line
    // hops charge the shared meter independently and must not be included here.
    baseline.saturating_add(work_budget::unordered_pair_count(edges))
}

fn swimlane_layout_preflight_work_units(nodes: usize, edges: usize) -> usize {
    // Keep preflight as a pure fail-fast estimate. In addition to the core-layout cost, reserve
    // one unordered-pair allowance for the direction post-processing that follows. That phase
    // charges its inspected candidates precisely, so this allowance is intentionally not charged
    // by `swimlane_core_layout_work_units`.
    swimlane_core_layout_work_units(nodes, edges)
        .saturating_add(work_budget::unordered_pair_count(edges))
}

#[cfg(test)]
mod tests {
    use super::{
        deterministic_identifier_cmp, swimlane_core_layout_work_units,
        swimlane_layout_preflight_work_units,
    };

    #[test]
    fn identifier_order_is_stable_utf16_code_unit_order() {
        let mut ids = [
            "B", "a", "A", "b", "a.1", "a-1", "a_1", "Z", "z", "é", "e", "中", "\u{e000}", "😀",
            "🧪",
        ];
        ids.sort_by(|left, right| deterministic_identifier_cmp(left, right));
        assert_eq!(
            ids,
            [
                "A", "B", "Z", "a", "a-1", "a.1", "a_1", "b", "e", "z", "é", "中", "😀", "🧪",
                "\u{e000}",
            ]
        );
    }

    #[test]
    fn core_layout_cost_accounts_routing_pairs_once() {
        // Four nodes plus three edges consume 31 linear units, including occurrence preparation;
        // routing can inspect three unordered edge pairs.
        assert_eq!(swimlane_core_layout_work_units(4, 3), 34);
    }

    #[test]
    fn preflight_reserves_direction_pairs_without_charging_them_to_core() {
        let core = swimlane_core_layout_work_units(4, 3);
        let preflight = swimlane_layout_preflight_work_units(4, 3);

        assert_eq!(core, 34);
        assert_eq!(preflight, 37);
    }

    #[test]
    fn swimlane_layout_work_estimates_saturate() {
        assert_eq!(
            swimlane_core_layout_work_units(usize::MAX, usize::MAX),
            usize::MAX
        );
        assert_eq!(
            swimlane_layout_preflight_work_units(usize::MAX, usize::MAX),
            usize::MAX
        );
    }
}

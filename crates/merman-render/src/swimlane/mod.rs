#[cfg(any(feature = "diagram-er", feature = "diagram-mindmap"))]
mod flat;

#[cfg(feature = "diagram-mindmap")]
pub(crate) use flat::layout_flat;
#[cfg(feature = "diagram-er")]
pub(crate) use flat::{MeasuredNode, layout_compound};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
mod prepare;
#[cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
mod shoal;
#[cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
pub(crate) use shoal::SwimlaneShoal;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use crate::LayoutExecution;
use crate::Result;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use crate::flowchart::{FlowchartConfigView, FlowchartEdgeTransportPlan, FlowchartRenderModelRef};
use crate::model::{
    Bounds, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneLayout, SwimlaneNodeLayout,
};
use crate::resources::OperationWorkMeter;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_model::presentation::PresentationConfig;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_render_flow::swimlane::lanes;
use merman_render_flow::swimlane::{admit_layout_work, config, source_layout_work_units, working};
use std::sync::Arc;

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
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) fn layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    svg_label_sidecar: Option<&crate::flowchart::FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<SwimlaneLayout> {
    let work_meter = execution.work_meter();
    let source_nodes = model.nodes.len().saturating_add(model.subgraphs.len());
    let source_work = source_layout_work_units(source_nodes, model.edges.len());
    work_meter.preflight(source_work)?;
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let visible_edges = render_model.projected_edges().collect::<Vec<_>>();
    #[cfg(feature = "layout-shoal")]
    let canonical = execution.layout_backend() == crate::layout_backend::GraphLayoutBackend::Shoal;
    #[cfg(not(feature = "layout-shoal"))]
    let canonical = false;
    if canonical {
        work_meter.charge(source_work)?;
    } else {
        admit_layout_work(&work_meter, source_work, visible_edges.len())?;
    }
    let transport_plan = FlowchartEdgeTransportPlan::for_keyed_edges(
        visible_edges
            .iter()
            .map(|(key, edge)| (*key, edge.semantic)),
    );
    let mut config = config::SwimlaneConfig::from_config(effective_config);
    if let Some(density) = svg_label_sidecar.and_then(|labels| labels.native_density()) {
        let (_, spacing) = merman_render_flow::layout_config::density_scales(density);
        config.node_gap *= spacing;
        config.layer_gap *= spacing;
    }
    let mut working = prepare::prepare(
        &render_model,
        (&visible_edges, &transport_plan),
        effective_config,
        #[cfg(feature = "layout-shoal")]
        canonical,
        execution,
        svg_label_sidecar,
        edge_style_plan,
    )?;
    #[cfg(feature = "layout-shoal")]
    let shoal = if canonical {
        Some(shoal::solve(
            &mut working,
            &render_model,
            &visible_edges,
            effective_config,
            config,
            execution,
            svg_label_sidecar,
        )?)
    } else {
        None
    };
    if !canonical {
        run_layout_core(&mut working, config, work_meter)?;
        prepare::measure_zero_width_lane_titles(
            &mut working,
            model,
            render_label_sources,
            effective_config,
            execution.text_measurer(),
            svg_label_sidecar,
        );
    }

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
        .or(config_curve)
        .unwrap_or("basis");
    if working.original_edges.len() != visible_edges.len() {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "Swimlane edge-owner count {} does not match visible edge count {}",
                working.original_edges.len(),
                visible_edges.len()
            ),
        });
    }

    let mut curve_by_id = std::collections::HashMap::with_capacity(visible_edges.len());
    for ((key, source), edge) in visible_edges.iter().zip(&working.original_edges) {
        if transport_plan.id(*key) != Some(edge.id.as_str())
            || edge.reference_id != edge.id
            || edge.from != source.from
            || edge.to != source.to
        {
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "Swimlane working edge `{}` does not match semantic owner {}",
                    edge.id,
                    key.semantic_index()
                ),
            });
        }
        let curve = source
            .semantic
            .interpolate
            .as_deref()
            .filter(|curve| !curve.is_empty())
            .unwrap_or(default_curve);
        curve_by_id.insert(edge.id.as_str(), curve);
    }
    let mut layout = project_layout(&working, &curve_by_id);
    #[cfg(feature = "layout-shoal")]
    {
        layout.shoal = shoal;
    }
    for (edge, (_, source)) in layout.edges.iter_mut().zip(&visible_edges) {
        if edge.id != source.semantic.id {
            edge.id.clone_from(&source.semantic.id);
        }
    }
    layout.edge_owners = crate::flowchart::FlowchartEdgeOwners::new(
        visible_edges.iter().map(|(key, _)| *key).collect(),
    );
    Ok(layout)
}

fn run_layout_core(
    working: &mut working::WorkingLayout,
    config: config::SwimlaneConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<()> {
    // Keep BPMN outlines tied to this renderer's Flowchart/Swimlane selectors, even if a
    // different consumer enables additional features on the shared solver crate.
    let use_bpmn_drawn_size = cfg!(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    ));
    merman_render_flow::swimlane::solve(working, config, use_bpmn_drawn_size, work_meter)
}

fn project_layout(
    working: &working::WorkingLayout,
    curve_by_id: &std::collections::HashMap<&str, &str>,
) -> SwimlaneLayout {
    let bounds = output_bounds(working);
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
    let edges = working
        .original_edges
        .iter()
        .map(|edge| SwimlaneEdgeLayout {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            points: edge.points.clone(),
            label_node_id: edge.label_node_id.clone(),
            reversed_for_layout: edge.reversed_for_layout,
            curve: match curve_by_id
                .get(edge.id.as_str())
                .copied()
                .unwrap_or("basis")
            {
                "basis" => "rounded",
                curve => curve,
            }
            .to_string(),
        })
        .collect();

    SwimlaneLayout {
        direction: working.direction,
        nodes,
        lanes,
        edges,
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        edge_owners: crate::flowchart::FlowchartEdgeOwners::default(),
        #[cfg(all(
            feature = "layout-shoal",
            any(feature = "diagram-flowchart", feature = "diagram-swimlane")
        ))]
        shoal: None,
        bounds,
    }
}

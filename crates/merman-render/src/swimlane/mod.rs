#[cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
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
#[cfg(any(feature = "diagram-er", feature = "diagram-mindmap"))]
use crate::model::LayoutEdge;
#[cfg(feature = "diagram-mindmap")]
use crate::model::LayoutNode;
#[cfg(feature = "diagram-er")]
use crate::model::SwimlaneDirection;
use crate::model::SwimlaneLayout;
#[cfg(any(feature = "diagram-er", feature = "diagram-mindmap"))]
use crate::resources::OperationWorkMeter;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-mindmap",
    feature = "diagram-swimlane"
))]
use merman_model::presentation::PresentationConfig;
#[cfg(feature = "diagram-er")]
pub(crate) use merman_render_flow::swimlane::MeasuredNode;
use merman_render_flow::swimlane::SwimlaneLayoutParts;
#[cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
use merman_render_flow::swimlane::working;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_render_flow::swimlane::{
    DefaultLaneIdPolicy, FlowSwimlanePreparation, admit_layout_work, config,
    measure_zero_width_lane_titles, prepare_working_layout, project_layout, solve,
    source_layout_work_units,
};
#[cfg(any(feature = "diagram-er", feature = "diagram-mindmap"))]
use std::sync::Arc;

// The facade's Flow selectors own BPMN outlines even when other crates unify Flow features.
const USE_BPMN_DRAWN_SIZE: bool = cfg!(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
));

#[cfg(feature = "diagram-mindmap")]
pub(crate) fn layout_flat(
    measured: &[(&LayoutNode, &str)],
    edges: &[LayoutEdge],
    config: &PresentationConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    merman_render_flow::swimlane::layout_flat(
        measured,
        edges,
        config,
        USE_BPMN_DRAWN_SIZE,
        work_meter,
    )
    .map(assemble_layout)
}

#[cfg(feature = "diagram-er")]
pub(crate) fn layout_compound(
    measured: &[MeasuredNode<'_>],
    edges: &[LayoutEdge],
    direction: SwimlaneDirection,
    config: &serde_json::Value,
    spacing: (f64, f64),
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    merman_render_flow::swimlane::layout_compound(
        measured,
        edges,
        direction,
        config,
        spacing,
        USE_BPMN_DRAWN_SIZE,
        work_meter,
    )
    .map(assemble_layout)
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
    #[cfg(feature = "layout-shoal")]
    let canonical = execution.layout_backend() == crate::layout_backend::GraphLayoutBackend::Shoal;
    #[cfg(not(feature = "layout-shoal"))]
    let canonical = false;
    let config_view = FlowchartConfigView::new(effective_config.as_value());
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let render_model = if canonical {
        render_model.with_shoal_svg_text(
            config_view.node_html_labels(),
            config_view.effective_html_labels(),
        )
    } else {
        render_model
    };
    let visible_edges = render_model.projected_edges().collect::<Vec<_>>();
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
    let config_curve = config_view.render_curve();
    let source_curve = model
        .edge_defaults
        .as_ref()
        .and_then(|defaults| defaults.interpolate.as_deref())
        .filter(|curve| !curve.is_empty());
    let default_curve = source_curve.or(config_curve).unwrap_or("basis");
    #[cfg(feature = "layout-shoal")]
    let default_curve = if canonical {
        execution
            .native_family_theme()
            .and_then(|theme| {
                crate::flowchart::native_theme::default_curve(
                    theme.form.edge_style,
                    model,
                    effective_config,
                )
            })
            .unwrap_or_else(|| {
                if default_curve == "basis"
                    && source_curve.is_none()
                    && !effective_config.explicit_config_owns_path("flowchart.curve")
                {
                    "rounded"
                } else {
                    default_curve
                }
            })
    } else {
        default_curve
    };
    let mut working = prepare_working_layout(
        &render_model,
        (&visible_edges, &transport_plan),
        FlowSwimlanePreparation {
            effective_config,
            work_meter: execution.work_meter_ref(),
            measurer: execution.text_measurer(),
            math_renderer: execution.math_renderer(),
            svg_label_sidecar,
            edge_style_plan,
            default_lane_id_policy: if canonical {
                DefaultLaneIdPolicy::ReserveCollision
            } else {
                DefaultLaneIdPolicy::Legacy
            },
        },
    )?;
    #[cfg(feature = "layout-shoal")]
    let shoal = if canonical {
        Some(shoal::solve(
            &mut working,
            &render_model,
            &visible_edges,
            effective_config,
            (
                config,
                crate::graph_layout::route_paint::RouteStyle::from_interpolate(default_curve, 0.0)
                    .routing_style(),
            ),
            execution,
            svg_label_sidecar,
        )?)
    } else {
        None
    };
    if !canonical {
        solve(&mut working, config, USE_BPMN_DRAWN_SIZE, work_meter)?;
        measure_zero_width_lane_titles(
            &mut working,
            model,
            render_label_sources,
            effective_config,
            execution.text_measurer(),
            svg_label_sidecar,
        );
    }

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
        let Some(transport_id) = transport_plan.id(*key).filter(|&id| {
            id == edge.id.as_str()
                && edge.reference_id == edge.id
                && edge.from == source.from
                && edge.to == source.to
        }) else {
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "Swimlane working edge `{}` does not match semantic owner {}",
                    edge.id,
                    key.semantic_index()
                ),
            });
        };
        let curve = source
            .semantic
            .interpolate
            .as_deref()
            .filter(|curve| !curve.is_empty())
            .unwrap_or(default_curve);
        curve_by_id.insert(transport_id, curve);
    }
    let mut layout = assemble_layout(project_layout(working, &curve_by_id));
    #[cfg(feature = "layout-shoal")]
    {
        layout.shoal = shoal;
    }
    for (edge, (_, source)) in layout.edges.iter_mut().zip(&visible_edges) {
        // Canonical painting retains the explicitly selected basis curve.
        if canonical && curve_by_id.get(edge.id.as_str()) == Some(&"basis") {
            edge.curve = "basis".to_owned();
        }
        if edge.id != source.semantic.id {
            edge.id.clone_from(&source.semantic.id);
        }
    }
    layout.edge_owners = crate::flowchart::FlowchartEdgeOwners::new(
        visible_edges.iter().map(|(key, _)| *key).collect(),
    );
    Ok(layout)
}

fn assemble_layout(
    (direction, nodes, lanes, edges, bounds): SwimlaneLayoutParts,
) -> SwimlaneLayout {
    SwimlaneLayout {
        direction,
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

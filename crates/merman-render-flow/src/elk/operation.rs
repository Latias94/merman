use std::sync::Arc;

use merman_layout_elk as elk;
use merman_model::{
    families::flowchart::{FlowchartModel, FlowchartRenderContext},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, Result, elk_adapter::feedback_edges::orient_feedback_edges,
    layout_work::OperationLayoutWorkControl, math::MathRenderer, resources::OperationWorkMeter,
    text::TextMeasurer,
};

use super::{
    FlowchartElkProjection, build_flowchart_elk_graph_with_render_labels_and_work_control,
    project_layout,
};
use crate::{
    class_style::FlowchartClassDefinitions,
    layout_config::FlowchartConfigView,
    projection::{FlowchartEdgeKey, FlowchartEdgeTransportPlan, FlowchartRenderModelRef},
    style::FlowchartEdgeStylePlan,
    svg_label::FlowchartSvgLabelSidecarBuilder,
};

/// Operation-owned resources and the selected ELK provider for this layout.
pub struct FlowchartElkLayoutExecution<'a> {
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    algorithm: elk::Algorithm,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
}

impl<'a> FlowchartElkLayoutExecution<'a> {
    pub fn new(
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        operation_seed: elk::ElkOperationSeed,
        algorithm: elk::Algorithm,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        edge_style_plan: &'a FlowchartEdgeStylePlan,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        Self {
            measurer,
            math_renderer,
            operation_seed,
            algorithm,
            svg_label_sidecar,
            edge_style_plan,
            work_meter,
        }
    }
}

/// Builds, solves, and projects the Flow graph under one layout work control.
pub fn layout(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    execution: FlowchartElkLayoutExecution<'_>,
) -> Result<FlowchartElkProjection> {
    let mut work_control = OperationLayoutWorkControl::new(execution.work_meter);
    let mut graph = build_flowchart_elk_graph_with_render_labels_and_work_control(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        execution.algorithm,
        execution.measurer,
        execution.math_renderer,
        execution.svg_label_sidecar,
        execution.edge_style_plan,
        Some(&mut work_control),
    )?;
    // Graph diagnostics expose Mermaid's raw edge ids. Bind transport ids only after
    // preparation, using the work tranche already charged there.
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    bind_flowchart_elk_transport_ids(render_model, &mut graph)?;
    let orientation =
        orient_feedback_edges(&mut graph, effective_config, &mut Some(&mut work_control))?;
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        elk::layout_with_operation_seed_and_work_control(
            orientation.graph(),
            execution.operation_seed,
            &mut work_control,
        )
    };
    let mut layout = solved.map_err(|error| work_control.map_elk_error(error))?;
    orientation.restore(&mut layout, &mut Some(&mut work_control))?;
    let settings = FlowchartConfigView::new(effective_config).layout_settings();
    project_layout(
        render_model,
        settings,
        &graph,
        layout,
        Some(&mut work_control),
    )
}

/// Solves an already measured diagnostic graph with the provider's raw seed policy.
#[cfg(any(test, feature = "test-support"))]
pub fn layout_diagnostic(
    model: FlowchartRenderModelRef<'_>,
    effective_config: &PresentationConfig,
    mut graph: elk::Graph,
) -> Result<FlowchartElkProjection> {
    bind_flowchart_elk_transport_ids(model, &mut graph)?;
    let mut no_work = None;
    let orientation = orient_feedback_edges(&mut graph, effective_config, &mut no_work)?;
    let mut layout = elk::layout(orientation.graph()).map_err(|error| Error::InvalidModel {
        message: format!("ELK layout failed: {error}"),
    })?;
    orientation.restore(&mut layout, &mut no_work)?;
    let settings = FlowchartConfigView::new(effective_config).layout_settings();
    project_layout(model, settings, &graph, layout, None)
}

fn bind_flowchart_elk_transport_ids(
    model: FlowchartRenderModelRef<'_>,
    graph: &mut elk::Graph,
) -> Result<()> {
    let projected_edges = model
        .projected_edges()
        .map(|(key, edge)| (key.semantic_index(), edge))
        .collect::<Vec<_>>();
    if graph.edges.len() != projected_edges.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "ELK graph edge count {} does not match projected edge count {}",
                graph.edges.len(),
                projected_edges.len()
            ),
        });
    }

    let transport_plan = FlowchartEdgeTransportPlan::for_keyed_edges(
        projected_edges
            .iter()
            .map(|(semantic_index, edge)| (FlowchartEdgeKey::new(*semantic_index), edge.semantic)),
    );
    for (transport, (semantic_index, projected)) in
        graph.edges.iter_mut().zip(projected_edges.iter())
    {
        if transport.id != projected.semantic.id
            || transport.source != projected.from
            || transport.target != projected.to
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "ELK graph edge `{}` is not bound to semantic owner {} (`{}`)",
                    transport.id,
                    semantic_index,
                    model
                        .edges
                        .get(*semantic_index)
                        .map(|edge| edge.id.as_str())
                        .unwrap_or("<missing>")
                ),
            });
        }
        let key = FlowchartEdgeKey::new(*semantic_index);
        let id = transport_plan.id(key).ok_or_else(|| Error::InvalidModel {
            message: format!("missing ELK transport id for semantic owner {semantic_index}"),
        })?;
        if transport.id != id {
            id.clone_into(&mut transport.id);
        }
    }

    Ok(())
}

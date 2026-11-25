use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::FlowchartLayout;
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use crate::{Error, Result};
use merman_layout_elk as elk;
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
#[cfg(test)]
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use std::sync::Arc;

use super::{FlowchartClassDefinitions, FlowchartSvgLabelSidecarBuilder};

mod input;
mod projection;

use input::build_flowchart_elk_graph_with_render_labels_and_work_control;
use projection::flowchart_layout_from_elk_with_render_labels_and_work_control;

pub(crate) struct FlowchartElkLayoutExecution<'a> {
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    algorithm: ElkRootAlgorithm,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a crate::svg::FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
}

impl<'a> FlowchartElkLayoutExecution<'a> {
    pub(crate) fn new(
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        operation_seed: elk::ElkOperationSeed,
        algorithm: ElkRootAlgorithm,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        edge_style_plan: &'a crate::svg::FlowchartEdgeStylePlan,
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

#[cfg(test)]
pub(crate) fn layout_flowchart_elk_typed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    let render_label_sources = FlowchartRenderContext::default();
    let mut graph = build_flowchart_elk_graph_with_render_labels(
        model,
        &render_label_sources,
        effective_config,
        measurer,
        math_renderer,
    )?;
    bind_flowchart_elk_transport_ids(model, &render_label_sources, &mut graph)?;
    let mut no_work = None;
    let orientation = crate::elk_feedback_edges::orient_feedback_edges(
        &mut graph,
        effective_config,
        &mut no_work,
    )?;
    let mut layout = elk::layout(orientation.graph()).map_err(|err| Error::InvalidModel {
        message: format!("ELK layout failed: {err}"),
    })?;
    orientation.restore(&mut layout, &mut no_work)?;
    flowchart_layout_from_elk_with_render_labels(
        model,
        &render_label_sources,
        effective_config,
        &graph,
        layout,
    )
}

/// Lays out a Flowchart through ELK using the render operation's captured seed.
///
/// This is intentionally crate-private. The public typed function above is a raw diagnostic API
/// and fails closed when source configuration uses ELK's `randomSeed = 0` sentinel.
#[cfg(test)]
pub(crate) fn layout_flowchart_elk_typed_with_operation_seed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        work_meter.as_ref(),
    )?;
    layout_flowchart_elk_typed_with_render_labels_and_operation_seed(
        model,
        &FlowchartRenderContext::default(),
        &FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        FlowchartElkLayoutExecution::new(
            measurer,
            math_renderer,
            operation_seed,
            ElkRootAlgorithm::for_diagnostic(effective_config),
            None,
            &edge_style_plan,
            work_meter,
        ),
    )
}

pub(crate) fn layout_flowchart_elk_typed_with_render_labels_and_operation_seed(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    execution: FlowchartElkLayoutExecution<'_>,
) -> Result<FlowchartLayout> {
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
    // The graph builder is a public diagnostic seam and therefore preserves Mermaid's raw edge
    // ids. The operation-local ELK adapter binds occurrence-safe transport ids only after that
    // seam, using the linear work tranche charged at graph preparation.
    bind_flowchart_elk_transport_ids(model, render_label_sources, &mut graph)?;
    let orientation = {
        let mut layout_work = Some(&mut work_control);
        crate::elk_feedback_edges::orient_feedback_edges(
            &mut graph,
            effective_config,
            &mut layout_work,
        )?
    };
    let mut layout = match crate::layout_work::measure_layout_solve!(
        elk::layout_with_operation_seed_and_work_control(
            orientation.graph(),
            execution.operation_seed,
            &mut work_control,
        )
    ) {
        Ok(layout) => layout,
        Err(error) => return Err(work_control.map_elk_error(error)),
    };
    let mut layout_work = Some(&mut work_control);
    orientation.restore(&mut layout, &mut layout_work)?;
    flowchart_layout_from_elk_with_render_labels_and_work_control(
        model,
        render_label_sources,
        effective_config,
        &graph,
        layout,
        Some(&mut work_control),
    )
}

fn projected_flowchart_edges<'a>(
    model: &'a FlowchartModel,
    render_label_sources: &'a FlowchartRenderContext,
) -> Vec<(usize, super::ProjectedFlowchartEdge<'a>)> {
    model
        .edges
        .iter()
        .enumerate()
        .filter_map(|(semantic_index, edge)| {
            super::project_flowchart_edge(edge, render_label_sources)
                .map(|projected| (semantic_index, projected))
        })
        .collect()
}

fn bind_flowchart_elk_transport_ids(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    graph: &mut elk::Graph,
) -> Result<()> {
    let projected_edges = projected_flowchart_edges(model, render_label_sources);
    if graph.edges.len() != projected_edges.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "ELK graph edge count {} does not match projected edge count {}",
                graph.edges.len(),
                projected_edges.len()
            ),
        });
    }

    let transport_plan = crate::flowchart::FlowchartEdgeTransportPlan::for_keyed_edges(
        projected_edges.iter().map(|(semantic_index, edge)| {
            (
                crate::flowchart::FlowchartEdgeKey::new(*semantic_index),
                edge.semantic,
            )
        }),
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
        let key = crate::flowchart::FlowchartEdgeKey::new(*semantic_index);
        let id = transport_plan.id(key).ok_or_else(|| Error::InvalidModel {
            message: format!("missing ELK transport id for semantic owner {semantic_index}"),
        })?;
        if transport.id != id {
            id.clone_into(&mut transport.id);
        }
    }

    Ok(())
}

#[cfg(test)]
fn flowchart_layout_from_elk(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
) -> Result<FlowchartLayout> {
    flowchart_layout_from_elk_with_render_labels(
        model,
        &FlowchartRenderContext::default(),
        effective_config,
        graph,
        layout,
    )
}

#[cfg(test)]
fn flowchart_layout_from_elk_with_render_labels(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
) -> Result<FlowchartLayout> {
    flowchart_layout_from_elk_with_render_labels_and_work_control(
        model,
        render_label_sources,
        effective_config,
        graph,
        layout,
        None,
    )
}

#[cfg(test)]
fn flowchart_layout_from_elk_with_work_control(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
    work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<FlowchartLayout> {
    flowchart_layout_from_elk_with_render_labels_and_work_control(
        model,
        &FlowchartRenderContext::default(),
        effective_config,
        graph,
        layout,
        work_control,
    )
}

/// Builds the ELK input graph from paired immutable Flowchart presentation data.
///
/// Flowchart labels retain source spelling that is not recoverable from the public semantic
/// model alone (for example an authored `&lt;` versus a literal `<`). Accepting the parsed artifact
/// keeps those declared facts paired with the effective configuration and prevents diagnostics from
/// silently measuring a different createText payload than the normal render pipeline.
#[cfg(test)]
pub(crate) fn build_flowchart_elk_graph(
    parsed: &PresentationDiagram,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<elk::Graph> {
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        return Err(Error::InvalidModel {
            message: format!(
                "expected Flowchart render model, got {}",
                parsed.model().kind()
            ),
        });
    };
    let empty_sources = FlowchartRenderContext::default();
    let render_label_sources = parsed
        .render_context()
        .flowchart_render_context()
        .unwrap_or(&empty_sources);
    build_flowchart_elk_graph_with_render_labels(
        model,
        render_label_sources,
        &parsed.metadata().effective_config,
        measurer,
        math_renderer,
    )
}

#[cfg(test)]
fn build_flowchart_elk_graph_from_semantic(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<elk::Graph> {
    build_flowchart_elk_graph_with_render_labels(
        model,
        &FlowchartRenderContext::default(),
        effective_config,
        measurer,
        math_renderer,
    )
}

#[cfg(test)]
fn build_flowchart_elk_graph_with_render_labels(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<elk::Graph> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        &OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ),
    )?;
    build_flowchart_elk_graph_with_render_labels_and_work_control(
        model,
        render_label_sources,
        &FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        ElkRootAlgorithm::for_diagnostic(effective_config),
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        None,
    )
}

#[cfg(test)]
fn build_flowchart_elk_graph_with_work_control(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        &OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ),
    )?;
    build_flowchart_elk_graph_with_render_labels_and_work_control(
        model,
        &FlowchartRenderContext::default(),
        &FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        ElkRootAlgorithm::for_diagnostic(effective_config),
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        work_control,
    )
}

fn charge_adapter_work(
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
    units: usize,
) -> Result<()> {
    match work_control.as_deref_mut() {
        Some(work_control) => work_control.charge_adapter(units),
        None => Ok(()),
    }
}

fn checked_adapter_add(
    work_control: &Option<&mut OperationLayoutWorkControl>,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right).ok_or_else(|| {
        work_control
            .as_deref()
            .map(|work_control| work_control.arithmetic_overflow())
            .unwrap_or_else(|| {
                OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .arithmetic_overflow()
            })
            .into()
    })
}

fn checked_adapter_mul(
    work_control: &Option<&mut OperationLayoutWorkControl>,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_mul(right).ok_or_else(|| {
        work_control
            .as_deref()
            .map(|work_control| work_control.arithmetic_overflow())
            .unwrap_or_else(|| {
                OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .arithmetic_overflow()
            })
            .into()
    })
}

#[cfg(test)]
mod browser_measurements_tests;

#[cfg(test)]
mod tests;

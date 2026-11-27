#[cfg(test)]
use crate::{
    Error, layout_backend::ElkRootAlgorithm, layout_work::OperationLayoutWorkControl,
    math::MathRenderer, resources::OperationWorkMeter, text::TextMeasurer,
};
use crate::{Result, model::FlowchartLayout};
#[cfg(test)]
use merman_layout_elk as elk;
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
#[cfg(test)]
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
pub(crate) use merman_render_flow::elk::FlowchartElkLayoutExecution;
#[cfg(test)]
use merman_render_flow::elk::build_flowchart_elk_graph_with_render_labels_and_work_control;
#[cfg(test)]
use std::sync::Arc;

use super::FlowchartClassDefinitions;
#[cfg(test)]
use super::{FlowchartRenderModelRef, FlowchartSvgLabelSidecarBuilder};

mod projection;

use projection::assemble_layout;
#[cfg(test)]
use projection::flowchart_layout_from_elk_with_render_labels_and_work_control;

#[cfg(test)]
pub(crate) fn layout_flowchart_elk_typed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    let render_label_sources = FlowchartRenderContext::default();
    let graph = build_flowchart_elk_graph_with_render_labels(
        model,
        &render_label_sources,
        effective_config,
        measurer,
        math_renderer,
    )?;
    merman_render_flow::elk::layout_diagnostic(
        FlowchartRenderModelRef::new(model, &render_label_sources),
        effective_config,
        graph,
    )
    .map(assemble_layout)
}

/// Lays out a Flowchart through ELK using the render operation's captured seed.
///
/// The raw diagnostic helper uses the provider's own seed policy, including rejection of
/// `randomSeed = 0`.
#[cfg(test)]
pub(crate) fn layout_flowchart_elk_typed_with_operation_seed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
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
            ElkRootAlgorithm::for_diagnostic(effective_config).algorithm(),
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
    merman_render_flow::elk::layout(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        execution,
    )
    .map(assemble_layout)
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
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
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
        ElkRootAlgorithm::for_diagnostic(effective_config).algorithm(),
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
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
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
        ElkRootAlgorithm::for_diagnostic(effective_config).algorithm(),
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        work_control,
    )
}

#[cfg(test)]
mod browser_measurements_tests;

#[cfg(test)]
mod tests;

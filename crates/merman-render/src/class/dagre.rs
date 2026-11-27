//! Class Dagre adapters for the renderer's shared createText metrics.

use super::{ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig};
use crate::Result;
use crate::graph_label::flowchart_label_metrics_for_layout;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::ClassDiagramLayout;
use crate::text::TextMeasurer;
use dugong::graphlib::Graph;
use dugong::{EdgeLabel, GraphLabel, NodeLabel};
use merman_model::presentation::PresentationConfig;
use merman_render_graph::class::dagre;

#[cfg(test)]
mod tests;

pub(crate) fn layout(
    model: &ClassDiagramModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render_config: &ClassRenderConfig,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    dagre::layout(
        model,
        effective_config,
        measurer,
        math_renderer,
        render_config,
        flowchart_label_metrics_for_layout,
        work_control,
    )
}

/// Debug-only helper: builds the production Class Dagre graph and returns it before layout runs.
///
/// Shares measurement and the Dagre projection with production, including source order,
/// named multiedges, terminal-label metrics, and namespace parents.
pub fn debug_build_class_diagram_dagre_graph(
    model: &ClassDiagramModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
) -> Result<Graph<NodeLabel, EdgeLabel, GraphLabel>> {
    dagre::debug_build_graph(
        model,
        effective_config,
        measurer,
        flowchart_label_metrics_for_layout,
    )
}

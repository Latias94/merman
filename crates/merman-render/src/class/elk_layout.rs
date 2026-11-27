//! Class ELK adapter for renderer-owned text measurement and backend selection.

use super::{ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig};
use crate::Result;
use crate::graph_label::flowchart_label_metrics_for_layout;
use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::ClassDiagramLayout;
use crate::text::TextMeasurer;
use merman_layout_elk::ElkOperationSeed;
use merman_model::presentation::PresentationConfig;

#[allow(
    clippy::too_many_arguments,
    reason = "Class ELK receives immutable prepared settings and operation-owned execution inputs."
)]
pub(crate) fn layout_class_diagram_elk_typed_with_config_and_operation_seed(
    model: &ClassDiagramModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: ElkOperationSeed,
    algorithm: ElkRootAlgorithm,
    render_config: &ClassRenderConfig,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    merman_render_graph::class::elk_layout::layout(
        model,
        effective_config,
        measurer,
        math_renderer,
        operation_seed,
        algorithm.algorithm(),
        render_config,
        flowchart_label_metrics_for_layout,
        work_control,
    )
}

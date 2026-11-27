//! Supplies the renderer's shared createText metrics to Class measurement.

#[cfg(feature = "layout-elk")]
use super::{ClassDiagramModel, ClassRenderConfig};
#[cfg(feature = "layout-elk")]
use crate::{
    Result, layout_work::OperationLayoutWorkControl, math::MathRenderer, text::TextMeasurer,
};
#[cfg(feature = "layout-elk")]
use merman_model::presentation::PresentationConfig;
#[cfg(feature = "layout-elk")]
use merman_render_graph::class::measured::{self, MeasuredClassDiagram};

#[cfg(feature = "layout-shoal")]
mod shoal;
#[cfg(feature = "layout-shoal")]
pub(super) use shoal::measure_for_shoal;

#[cfg(feature = "layout-elk")]
pub(super) fn measure_class_diagram(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render: &ClassRenderConfig,
    uses_elk_adapter: bool,
    work: Option<&OperationLayoutWorkControl>,
) -> Result<MeasuredClassDiagram> {
    measured::measure_class_diagram(
        model,
        config,
        measurer,
        math_renderer,
        render,
        uses_elk_adapter,
        crate::graph_label::flowchart_label_metrics_for_layout,
        work,
    )
}

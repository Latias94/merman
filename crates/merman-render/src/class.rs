//! Class layout providers and compatibility exports.

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid",
    feature = "layout-layered",
    feature = "layout-shoal"
))]
use merman_render_graph::class::ClassDiagramModel;
pub(crate) use merman_render_graph::class::{
    ClassNodeVisualPlan, ClassPreparedLabels, ClassRelationThemePlan, ClassRenderConfig,
    ClassTextThemePlan, class_layout_work_units, class_requires_math,
};
#[cfg(feature = "layout-shoal")]
use merman_render_graph::class::{class_namespace_label, normalize_dir};

#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk_layout;
#[cfg(feature = "layout-shoal")]
mod measured;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;

#[doc(hidden)]
#[cfg(feature = "layout-dagre")]
pub use dagre::debug_build_class_diagram_dagre_graph;
#[cfg(feature = "layout-dagre")]
pub(crate) use dagre::layout as layout_class_diagram_typed_with_config;
#[cfg(feature = "layout-elk")]
pub(crate) use elk_layout::layout_class_diagram_elk_typed_with_config_and_operation_seed;

#[cfg(feature = "layout-layered")]
pub(crate) fn layout_layered(
    model: &ClassDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
    render: &ClassRenderConfig,
    work: &mut crate::layout_work::OperationLayoutWorkControl,
) -> crate::Result<(
    crate::model::ClassDiagramLayout,
    ClassPreparedLabels,
    crate::graph_layout::RouteContract,
)> {
    if let Some((layout, labels)) = merman_render_graph::class::layered::layout(
        model,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        render,
        crate::graph_label::flowchart_label_metrics_for_layout,
        work,
        execution.work_meter_ref(),
    )? {
        return Ok((layout, labels, Default::default()));
    }

    work.checkpoint()?;
    #[cfg(feature = "layout-dagre")]
    if execution
        .family
        .session()
        .supports_capability(crate::RenderCapability::LayoutDagre)
    {
        work.charge_adapter(class_layout_work_units(model, work)?)?;
        let (layout, mut labels) = layout_class_diagram_typed_with_config(
            model,
            config,
            execution.text_measurer(),
            execution.math_renderer(),
            render,
            work,
        )?;
        labels.mark_layered_dagre_fallback();
        return Ok((layout, labels, Default::default()));
    }
    Err(crate::Error::MissingCapability {
        capability: crate::RenderCapability::LayoutDagre,
        diagram_type: model.model().diagram_type.clone(),
    })
}

#[cfg(feature = "layout-grid")]
pub(crate) fn layout_grid(
    model: &ClassDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
    render: &ClassRenderConfig,
    work: &crate::layout_work::OperationLayoutWorkControl,
) -> crate::Result<(
    crate::model::ClassDiagramLayout,
    ClassPreparedLabels,
    crate::graph_layout::RouteContract,
)> {
    merman_render_graph::class::grid::layout(
        model,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        render,
        crate::graph_label::flowchart_label_metrics_for_layout,
        work,
        execution.work_meter_ref(),
    )
    .map(|(layout, labels, paint)| {
        (
            layout,
            labels,
            crate::graph_layout::RouteContract::Grid(paint),
        )
    })
}

#[cfg(all(test, feature = "layout-dagre"))]
mod native_theme;
#[cfg(all(test, feature = "layout-dagre"))]
mod tests;

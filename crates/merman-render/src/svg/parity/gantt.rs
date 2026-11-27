use merman_model::families::gantt::GanttDiagramRenderModel;
use merman_render_core::{config::ConfigLookup, root_svg};

use super::SvgExecution;
use crate::gantt::{GanttStyle, GanttSvgContext, render_gantt_svg};
use crate::model::GanttDiagramLayout;
use crate::{DiagramFamilyId, Result};

pub(super) fn render_gantt_diagram_svg_model(
    layout: &GanttDiagramLayout,
    model: &GanttDiagramRenderModel,
    style: &GanttStyle,
    task_source_indices: &[usize],
    effective_config: &dyn ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    render_gantt_svg(
        layout,
        model,
        style,
        task_source_indices,
        effective_config,
        GanttSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::GANTT, diagram_id),
            local_time_zone: options.local_time_zone(),
            unix_ms: options.unix_ms(),
            work_meter: options.work_meter(),
        },
    )
}

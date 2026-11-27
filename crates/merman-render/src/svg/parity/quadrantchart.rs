use merman_model::{families::quadrant_chart::QuadrantChartDiagramModel, family::DiagramFamilyId};
use merman_render_charts::quadrantchart::{QuadrantChartSvgContext, render_quadrantchart_svg};
use merman_render_core::root_svg::RootedSvg;
use serde_json::Value;

use super::SvgExecution;
use crate::{
    Result, model::QuadrantChartDiagramLayout, quadrantchart::QuadrantChartPointThemePlan,
};

pub(super) fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    model: &QuadrantChartDiagramModel,
    point_theme: &QuadrantChartPointThemePlan,
    effective_config: &Value,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("quadrantchart");
    render_quadrantchart_svg(
        layout,
        model,
        point_theme,
        effective_config,
        &QuadrantChartSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::QUADRANT_CHART, diagram_id),
            work_meter: options.work_meter(),
        },
    )
}

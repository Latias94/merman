use super::SvgExecution;
use crate::{Result, model::TimelineDiagramLayout};
use merman_render_core::root_svg;

pub(super) fn render_timeline_diagram_svg_model(
    layout: &TimelineDiagramLayout,
    style: &crate::timeline::TimelineStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    crate::timeline::render_timeline_svg(
        layout,
        style,
        diagram_id,
        &options.root_viewport(crate::DiagramFamilyId::TIMELINE, diagram_id),
        options.work_meter(),
    )
}

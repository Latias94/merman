use super::SvgExecution;
use crate::{Result, model::CynefinDiagramLayout};
use merman_model::families::cynefin::CynefinDiagramModel;
use merman_render_core::root_svg;
use merman_render_structured::cynefin::{CynefinSvgContext, render_cynefin_svg};

pub(crate) fn render_cynefin_diagram_svg_model(
    layout: &CynefinDiagramLayout,
    model: &CynefinDiagramModel,
    diagram_title: Option<&str>,
    style: &crate::cynefin::CynefinStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("cynefin");
    render_cynefin_svg(
        layout,
        model,
        diagram_title,
        style,
        CynefinSvgContext {
            diagram_id,
            viewport: options.root_viewport(crate::DiagramFamilyId::CYNEFIN, diagram_id),
            work_meter: options.work_meter(),
        },
    )
}

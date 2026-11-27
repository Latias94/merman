use merman_model::families::journey::JourneyDiagramModel;
use merman_render_core::root_svg;

use super::SvgExecution;
use crate::journey::{JourneyStyle, render_journey_svg};
use crate::model::JourneyDiagramLayout;
use crate::{DiagramFamilyId, Result};

pub(super) fn render_journey_diagram_svg_model(
    layout: &JourneyDiagramLayout,
    model: &JourneyDiagramModel,
    style: &JourneyStyle,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    render_journey_svg(
        layout,
        model,
        style,
        diagram_title,
        diagram_id,
        &options.root_viewport(DiagramFamilyId::JOURNEY, diagram_id),
        options.work_meter(),
    )
}
